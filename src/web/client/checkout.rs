//! 微信支付 Native 下单与经过验签的异步通知。

use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1v15::{Signature, SigningKey, VerifyingKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey},
    signature::{SignatureEncoding, Signer, Verifier},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    config::WechatPayConfig,
    entity::{column_order, column_subscription, paid_column},
};

use super::{
    super::{ApiError, AppState, error},
    reader,
};

/// 微信 Native 下单响应；二维码内容仅用于引导支付，不构成访问凭据。
#[derive(Serialize, ToSchema)]
pub struct CheckoutResponse {
    /// 商户订单号。
    pub order_id: String,
    /// 微信扫码支付二维码内容。
    pub code_url: String,
}

/// 读者自己的订单状态，仅数据库已确认的 paid 才表示有权阅读。
#[derive(Serialize, ToSchema)]
pub struct OrderResponse {
    /// 商户订单号。
    pub order_id: String,
    /// pending 或 paid。
    pub status: String,
}

/// 微信 Native 接口的最小请求格式。
#[derive(Serialize)]
struct NativeOrder<'a> {
    appid: &'a str,
    mchid: &'a str,
    description: &'a str,
    out_trade_no: &'a str,
    notify_url: &'a str,
    amount: NativeAmount,
}

/// 人民币分，避免前端篡改价格。
#[derive(Serialize)]
struct NativeAmount {
    total: i32,
    currency: &'static str,
}

/// 微信 Native 返回的二维码 URL。
#[derive(Deserialize)]
struct NativeResult {
    code_url: String,
}

/// 微信异步通知的加密资源。
#[derive(Deserialize)]
struct NotifyEnvelope {
    event_type: String,
    resource: NotifyResource,
}

/// 回调资源使用 API v3 密钥进行 AES-256-GCM 解密。
#[derive(Deserialize)]
struct NotifyResource {
    algorithm: String,
    ciphertext: String,
    nonce: String,
    associated_data: String,
}

/// 解密后的支付事实，所有字段都与本地订单再次比对。
#[derive(Deserialize)]
struct PaidTransaction {
    out_trade_no: String,
    transaction_id: String,
    trade_state: String,
    mchid: String,
    appid: String,
    amount: PaidAmount,
}

/// 微信通知的实付金额。
#[derive(Deserialize)]
struct PaidAmount {
    total: i32,
    currency: String,
}

/// 使用商户 RSA 私钥签署请求，签名覆盖即将发送的原始 JSON 字节。
fn authorization(
    config: &WechatPayConfig,
    body: &str,
    timestamp: i64,
    nonce: &str,
) -> anyhow::Result<String> {
    let private = RsaPrivateKey::from_pkcs8_pem(&config.merchant_private_key)?;
    let message = format!("POST\n/v3/pay/transactions/native\n{timestamp}\n{nonce}\n{body}\n");
    let signature = SigningKey::<Sha256>::new(private).sign(message.as_bytes());
    Ok(format!(
        "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{}\",timestamp=\"{}\",serial_no=\"{}\",signature=\"{}\"",
        config.mch_id,
        nonce,
        timestamp,
        config.merchant_serial,
        STANDARD.encode(signature.to_bytes())
    ))
}

/// 验证平台签名及时间窗口，再解密支付通知。
fn verified_transaction(
    config: &WechatPayConfig,
    headers: &HeaderMap,
    body: &[u8],
) -> anyhow::Result<PaidTransaction> {
    let field = |name: &str| -> anyhow::Result<&str> {
        Ok(headers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("缺少微信通知头"))?
            .to_str()?)
    };
    if field("wechatpay-serial")? != config.platform_serial {
        anyhow::bail!("平台公钥标识不匹配")
    }
    let timestamp: i64 = field("wechatpay-timestamp")?.parse()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    if (now - timestamp).abs() > 300 {
        anyhow::bail!("通知时间超出允许窗口")
    }
    let nonce = field("wechatpay-nonce")?;
    let signature =
        Signature::try_from(STANDARD.decode(field("wechatpay-signature")?)?.as_slice())?;
    let public = RsaPublicKey::from_public_key_pem(&config.platform_public_key)?;
    let message = format!("{timestamp}\n{nonce}\n{}\n", std::str::from_utf8(body)?);
    VerifyingKey::<Sha256>::new(public).verify(message.as_bytes(), &signature)?;
    let envelope: NotifyEnvelope = serde_json::from_slice(body)?;
    if envelope.event_type != "TRANSACTION.SUCCESS"
        || envelope.resource.algorithm != "AEAD_AES_256_GCM"
    {
        anyhow::bail!("非成功支付通知")
    }
    let cipher = Aes256Gcm::new_from_slice(config.api_v3_key.as_bytes())
        .map_err(|_| anyhow::anyhow!("API v3 密钥无效"))?;
    if envelope.resource.nonce.len() != 12 {
        anyhow::bail!("资源随机数长度无效")
    }
    let encrypted = STANDARD.decode(envelope.resource.ciphertext)?;
    let nonce: [u8; 12] = envelope.resource.nonce.as_bytes().try_into()?;
    let plaintext = cipher
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: &encrypted,
                aad: envelope.resource.associated_data.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("通知解密失败"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

/// 已登录读者创建自己专栏的微信扫码订单。
#[utoipa::path(post, path = "/api/v1/columns/{slug}/checkout", params(("slug" = String, Path)), responses((status = 200, body = CheckoutResponse), (status = 401, body = ApiError), (status = 503, body = ApiError)), tag = "columns")]
pub async fn create(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match reader::require_reader(&state, &headers, true).await {
        Ok(session) => session,
        Err(response) => return response,
    };
    let Some(config) = state.wechat_pay.as_ref() else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_unavailable",
            "微信支付尚未配置",
        );
    };
    let mut redis = match state.redis.get_multiplexed_async_connection().await {
        Ok(redis) => redis,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            );
        }
    };
    let key = format!("reader-checkout:{}", session.reader_id);
    let attempts: i64 = match redis::cmd("INCR").arg(&key).query_async(&mut redis).await {
        Ok(value) => value,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            );
        }
    };
    if attempts == 1
        && redis::cmd("EXPIRE")
            .arg(&key)
            .arg(60)
            .query_async::<()>(&mut redis)
            .await
            .is_err()
    {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "服务暂时不可用",
        );
    }
    if attempts > 5 {
        return error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "请稍后重试");
    }
    let column = match paid_column::Entity::find()
        .filter(paid_column::Column::Slug.eq(slug))
        .filter(paid_column::Column::Visible.eq(true))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "column_not_found", "专栏不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取待购买专栏失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    match reader::owns_column(&state, session.reader_id, column.id).await {
        Ok(true) => return error(StatusCode::CONFLICT, "already_subscribed", "已购买本专栏"),
        Ok(false) => {}
        Err(err) => {
            tracing::error!(error = %err, "检查专栏权益失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    }
    let order_id = Uuid::new_v4().simple().to_string();
    let order = column_order::ActiveModel {
        id: Set(order_id.clone()),
        reader_id: Set(session.reader_id),
        paid_column_id: Set(column.id),
        amount_cents: Set(column.price_cents),
        status: Set("pending".into()),
        wechat_transaction_id: Set(None),
        created_at: Set(Utc::now()),
        paid_at: Set(None),
    };
    if let Err(err) = order.insert(&state.db).await {
        tracing::error!(error = %err, "建立微信订单失败");
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "服务暂时不可用",
        );
    }
    let body = serde_json::to_string(&NativeOrder {
        appid: &config.app_id,
        mchid: &config.mch_id,
        description: &column.title,
        out_trade_no: &order_id,
        notify_url: &config.notify_url,
        amount: NativeAmount {
            total: column.price_cents,
            currency: "CNY",
        },
    })
    .unwrap();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let nonce = Uuid::new_v4().simple().to_string();
    let auth = match authorization(config, &body, timestamp, &nonce) {
        Ok(auth) => auth,
        Err(err) => {
            tracing::error!(error = %err, "微信商户签名失败");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "payment_unavailable",
                "微信支付配置无效",
            );
        }
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "payment_unavailable",
                "微信支付暂不可用",
            );
        }
    };
    let result = client
        .post("https://api.mch.weixin.qq.com/v3/pay/transactions/native")
        .header("Authorization", auth)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await;
    let Ok(response) = result else {
        return error(StatusCode::BAD_GATEWAY, "payment_failed", "微信下单失败");
    };
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "微信下单被拒绝");
        return error(StatusCode::BAD_GATEWAY, "payment_failed", "微信下单失败");
    }
    match response.json::<NativeResult>().await {
        Ok(result) if result.code_url.starts_with("weixin://") => Json(CheckoutResponse {
            order_id,
            code_url: result.code_url,
        })
        .into_response(),
        _ => error(StatusCode::BAD_GATEWAY, "payment_failed", "微信下单失败"),
    }
}

/// 读者只能查询自己的订单；前端轮询不负责开通订阅。
#[utoipa::path(get, path = "/api/v1/reader/orders/{order_id}", params(("order_id" = String, Path)), responses((status = 200, body = OrderResponse), (status = 401, body = ApiError)), tag = "reader")]
pub async fn status(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match reader::require_reader(&state, &headers, false).await {
        Ok(session) => session,
        Err(response) => return response,
    };
    match column_order::Entity::find_by_id(order_id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) if row.reader_id == session.reader_id => {
            let mut response = Json(OrderResponse {
                order_id: row.id,
                status: row.status,
            })
            .into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
            response
        }
        Ok(_) => error(StatusCode::NOT_FOUND, "order_not_found", "订单不存在"),
        Err(err) => {
            tracing::error!(error = %err, "查询订单失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 只有验签、解密、核对金额及商户后才在同一事务内授予阅读权。
#[utoipa::path(post, path = "/api/v1/wechat/notify", responses((status = 204), (status = 401, body = ApiError)), tag = "columns")]
pub async fn notify(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(config) = state.wechat_pay.as_ref() else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_unavailable",
            "微信支付尚未配置",
        );
    };
    let transaction = match verified_transaction(config, &headers, &body) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(error = %err, "拒绝无效微信支付通知");
            return error(
                StatusCode::UNAUTHORIZED,
                "invalid_payment_notice",
                "通知验证失败",
            );
        }
    };
    if transaction.trade_state != "SUCCESS"
        || transaction.mchid != config.mch_id
        || transaction.appid != config.app_id
        || transaction.amount.currency != "CNY"
    {
        return error(
            StatusCode::UNAUTHORIZED,
            "invalid_payment_notice",
            "通知验证失败",
        );
    }
    let result = async {
        let db = state.db.begin().await?;
        let Some(order) = column_order::Entity::find_by_id(&transaction.out_trade_no)
            .lock_exclusive()
            .one(&db)
            .await?
        else {
            return Ok::<bool, sea_orm::DbErr>(false);
        };
        if order.amount_cents != transaction.amount.total || transaction.transaction_id.is_empty() {
            return Ok(false);
        }
        if order.status == "paid" {
            return Ok(
                order.wechat_transaction_id.as_deref() == Some(transaction.transaction_id.as_str())
            );
        }
        if order.status != "pending" {
            return Ok(false);
        }
        let already = column_subscription::Entity::find()
            .filter(column_subscription::Column::ReaderId.eq(order.reader_id))
            .filter(column_subscription::Column::PaidColumnId.eq(order.paid_column_id))
            .one(&db)
            .await?;
        if already.is_none() {
            column_subscription::ActiveModel {
                id: sea_orm::NotSet,
                reader_id: Set(order.reader_id),
                paid_column_id: Set(order.paid_column_id),
                created_at: Set(Utc::now()),
            }
            .insert(&db)
            .await?;
        }
        let mut active = order.into_active_model();
        active.status = Set("paid".into());
        active.wechat_transaction_id = Set(Some(transaction.transaction_id));
        active.paid_at = Set(Some(Utc::now()));
        active.update(&db).await?;
        db.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(
            StatusCode::BAD_REQUEST,
            "payment_mismatch",
            "订单信息不匹配",
        ),
        Err(err) => {
            tracing::error!(error = %err, "处理微信支付通知失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::Aead;
    use rsa::{
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
        rand_core::OsRng,
    };
    use std::sync::Arc;

    use crate::{
        agent::tools::ToolRegistry,
        infrastructure::{
            agent_skills::SkillStore, article_repository::SeaOrmArticleRepository,
            search::SearchEngine,
        },
    };

    /// 测试通知使用真实 RSA 签名和 AES-GCM 密文。
    fn signed_notice(
        config: &WechatPayConfig,
        private: &RsaPrivateKey,
        order: &str,
        amount: i32,
    ) -> (HeaderMap, Bytes) {
        let plaintext = serde_json::json!({
            "out_trade_no": order, "transaction_id": "test-wechat-transaction", "trade_state": "SUCCESS",
            "mchid": config.mch_id, "appid": config.app_id, "amount": {"total": amount, "currency": "CNY"},
        }).to_string();
        let cipher = Aes256Gcm::new_from_slice(config.api_v3_key.as_bytes()).unwrap();
        let encrypted = cipher
            .encrypt(
                &Nonce::from(*b"123456789012"),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: b"associated",
                },
            )
            .unwrap();
        let body = serde_json::json!({
            "event_type": "TRANSACTION.SUCCESS",
            "resource": {"algorithm": "AEAD_AES_256_GCM", "ciphertext": STANDARD.encode(encrypted),
                "nonce": "123456789012", "associated_data": "associated"},
        })
        .to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message = format!("{timestamp}\ntest-nonce\n{body}\n");
        let signature = SigningKey::<Sha256>::new(private.clone()).sign(message.as_bytes());
        let mut headers = HeaderMap::new();
        headers.insert("wechatpay-serial", config.platform_serial.parse().unwrap());
        headers.insert(
            "wechatpay-timestamp",
            timestamp.to_string().parse().unwrap(),
        );
        headers.insert("wechatpay-nonce", "test-nonce".parse().unwrap());
        headers.insert(
            "wechatpay-signature",
            STANDARD.encode(signature.to_bytes()).parse().unwrap(),
        );
        (headers, Bytes::from(body))
    }

    /// 使用独立测试密钥生成真实签名与密文，防止伪造回调绕过验签。
    #[test]
    fn accepts_only_signed_and_untampered_payment_notice() {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let config = WechatPayConfig {
            mch_id: "merchant".into(),
            app_id: "app".into(),
            merchant_serial: "merchant-key".into(),
            merchant_private_key: private.to_pkcs8_pem(LineEnding::LF).unwrap().to_string(),
            // 仅用于测试的 32 字节 AES 密钥，不是支付商户凭据。
            api_v3_key: "t".repeat(32),
            platform_public_key: private
                .to_public_key()
                .to_public_key_pem(LineEnding::LF)
                .unwrap(),
            platform_serial: "platform-key".into(),
            notify_url: "https://example.com/api/v1/wechat/notify".into(),
        };
        let plaintext = serde_json::json!({
            "out_trade_no": "test-order", "transaction_id": "wechat-transaction", "trade_state": "SUCCESS",
            "mchid": "merchant", "appid": "app", "amount": {"total": 100, "currency": "CNY"},
        }).to_string();
        let cipher = Aes256Gcm::new_from_slice(config.api_v3_key.as_bytes()).unwrap();
        // AES-GCM 随机数必须是可放入 JSON 的可打印字节。
        let nonce = *b"123456789012";
        let encrypted = cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: b"associated",
                },
            )
            .unwrap();
        let body = serde_json::json!({
            "event_type": "TRANSACTION.SUCCESS",
            "resource": { "algorithm": "AEAD_AES_256_GCM", "ciphertext": STANDARD.encode(encrypted),
                "nonce": "123456789012", "associated_data": "associated" },
        })
        .to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message = format!("{timestamp}\ntest-nonce\n{body}\n");
        let signature = SigningKey::<Sha256>::new(private).sign(message.as_bytes());
        let mut headers = HeaderMap::new();
        headers.insert("wechatpay-serial", "platform-key".parse().unwrap());
        headers.insert(
            "wechatpay-timestamp",
            timestamp.to_string().parse().unwrap(),
        );
        headers.insert("wechatpay-nonce", "test-nonce".parse().unwrap());
        headers.insert(
            "wechatpay-signature",
            STANDARD.encode(signature.to_bytes()).parse().unwrap(),
        );

        let verified = verified_transaction(&config, &headers, body.as_bytes()).unwrap();
        assert_eq!(verified.out_trade_no, "test-order");
        assert_eq!(verified.amount.total, 100);
        assert!(
            verified_transaction(
                &config,
                &headers,
                body.replace("TRANSACTION.SUCCESS", "TRANSACTION.FAIL")
                    .as_bytes()
            )
            .is_err()
        );
        headers.insert("wechatpay-serial", "other-key".parse().unwrap());
        assert!(verified_transaction(&config, &headers, body.as_bytes()).is_err());
    }

    /// 专用数据库验证伪造、金额不符、成功及重复通知对订阅的实际影响。
    #[tokio::test]
    #[ignore = "需要 PAID_TEST_DATABASE_URL 和 PAID_TEST_REDIS_URL 指向专用测试环境"]
    async fn payment_notice_grants_subscription_only_after_verified_success() {
        let db = sea_orm::Database::connect(
            std::env::var("PAID_TEST_DATABASE_URL").expect("测试数据库地址"),
        )
        .await
        .unwrap();
        let redis =
            redis::Client::open(std::env::var("PAID_TEST_REDIS_URL").expect("测试 Redis 地址"))
                .unwrap();
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let config = WechatPayConfig {
            mch_id: "merchant".into(),
            app_id: "app".into(),
            merchant_serial: "merchant-key".into(),
            merchant_private_key: private.to_pkcs8_pem(LineEnding::LF).unwrap().to_string(),
            // 仅用于测试的 32 字节 AES 密钥，不是支付商户凭据。
            api_v3_key: "t".repeat(32),
            platform_public_key: private
                .to_public_key()
                .to_public_key_pem(LineEnding::LF)
                .unwrap(),
            platform_serial: "platform-key".into(),
            notify_url: "https://example.com/api/v1/wechat/notify".into(),
        };
        let now = Utc::now();
        let suffix = Uuid::new_v4().simple().to_string();
        let column = paid_column::ActiveModel {
            visible: Set(true),
            id: sea_orm::NotSet,
            public_id: Set(Uuid::new_v4()),
            slug: Set(format!("pay-{suffix}")),
            title: Set("支付测试".into()),
            description: Set(String::new()),
            price_cents: Set(100),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();
        let reader = crate::entity::reader_user::ActiveModel {
            id: sea_orm::NotSet,
            public_id: Set(Uuid::new_v4()),
            username: Set(format!("payreader_{suffix}")),
            password_hash: Set("test-only".into()),
            created_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();
        let order_id = Uuid::new_v4().simple().to_string();
        column_order::ActiveModel {
            id: Set(order_id.clone()),
            reader_id: Set(reader.id),
            paid_column_id: Set(column.id),
            amount_cents: Set(100),
            status: Set("pending".into()),
            wechat_transaction_id: Set(None),
            created_at: Set(now),
            paid_at: Set(None),
        }
        .insert(&db)
        .await
        .unwrap();
        let state = AppState {
            tools: Arc::new(
                ToolRegistry::new(
                    std::env::temp_dir().join("oxide-pay-notice-tools.json"),
                    None,
                )
                .unwrap(),
            ),
            skills: Arc::new(SkillStore::new(
                std::env::temp_dir().join("oxide-pay-notice-skills"),
            )),
            articles: Arc::new(SeaOrmArticleRepository::new(db.clone())),
            db: db.clone(),
            redis,
            comment_hash_key: Arc::from("test-secret-with-at-least-32-bytes"),
            session_secure: false,
            public_base_url: Arc::from("http://127.0.0.1:3000"),
            search: Arc::new(SearchEngine::in_memory()),
            notion_api_key: None,
            wechat_pay: Some(Arc::new(config.clone())),
        };
        let count = || {
            column_subscription::Entity::find()
                .filter(column_subscription::Column::ReaderId.eq(reader.id))
                .filter(column_subscription::Column::PaidColumnId.eq(column.id))
        };
        let (headers, body) = signed_notice(&config, &private, &order_id, 100);
        let tampered = Bytes::from(
            String::from_utf8(body.to_vec())
                .unwrap()
                .replace("TRANSACTION.SUCCESS", "TRANSACTION.FAIL"),
        );
        assert_eq!(
            notify(State(state.clone()), headers.clone(), tampered)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert!(count().one(&db).await.unwrap().is_none());
        let (wrong_headers, wrong_body) = signed_notice(&config, &private, &order_id, 200);
        assert_eq!(
            notify(State(state.clone()), wrong_headers, wrong_body)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert!(count().one(&db).await.unwrap().is_none());
        assert_eq!(
            notify(State(state.clone()), headers.clone(), body.clone())
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        assert!(count().one(&db).await.unwrap().is_some());
        assert_eq!(
            notify(State(state), headers, body).await.status(),
            StatusCode::NO_CONTENT
        );
        let order = column_order::Entity::find_by_id(&order_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(order.status, "paid");
        assert_eq!(
            order.wechat_transaction_id.as_deref(),
            Some("test-wechat-transaction")
        );
        column_subscription::Entity::delete_many()
            .filter(column_subscription::Column::ReaderId.eq(reader.id))
            .exec(&db)
            .await
            .unwrap();
        column_order::Entity::delete_by_id(order_id)
            .exec(&db)
            .await
            .unwrap();
        crate::entity::reader_user::Entity::delete_by_id(reader.id)
            .exec(&db)
            .await
            .unwrap();
        paid_column::Entity::delete_by_id(column.id)
            .exec(&db)
            .await
            .unwrap();
    }
}
