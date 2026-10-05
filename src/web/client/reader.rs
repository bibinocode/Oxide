//! 读者注册、登录和独立于管理员的 Redis 会话。

use anyhow::{Context, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use cookie::{Cookie, SameSite};
use rand_core::OsRng;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::{column_subscription, reader_user};

use super::super::{ApiError, AppState, error};

/// Redis 保存的读者会话；Cookie 只有随机令牌。
#[derive(Clone, Serialize, Deserialize)]
pub struct ReaderSession {
    /// 服务端读者主键，不信任客户端自报身份。
    pub reader_id: i64,
    /// 页面展示名称。
    pub username: String,
    /// 写请求的防跨站令牌。
    pub csrf_token: String,
}

/// 注册与登录共享的凭据格式。
#[derive(Deserialize, ToSchema)]
pub struct ReaderCredentials {
    /// 3 至 32 位字母、数字或下划线。
    pub username: String,
    /// 至少 12 字符的密码。
    pub password: String,
}

/// 仅返回展示名称和写请求令牌。
#[derive(Serialize, ToSchema)]
pub struct ReaderSessionResponse {
    /// 登录名称。
    pub username: String,
    /// 提交订单和退出时使用的令牌。
    pub csrf_token: String,
}

/// 限制同一登录名一分钟内的尝试次数，Redis 不可用时停止认证写入。
async fn allow_attempt(state: &AppState, username: &str) -> Result<bool> {
    let mut redis = state.redis.get_multiplexed_async_connection().await?;
    let key = format!("reader-auth:{}", username.to_ascii_lowercase());
    let attempts: i64 = redis::cmd("INCR").arg(&key).query_async(&mut redis).await?;
    if attempts == 1 {
        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(60)
            .query_async::<()>(&mut redis)
            .await?;
    }
    Ok(attempts <= 10)
}

/// 将读者会话写入 Redis 并设置独立 Cookie。
async fn create_session(state: &AppState, row: reader_user::Model) -> Result<Response> {
    let token = Uuid::new_v4().to_string();
    let session = ReaderSession {
        reader_id: row.id,
        username: row.username,
        csrf_token: Uuid::new_v4().to_string(),
    };
    let mut redis = state.redis.get_multiplexed_async_connection().await?;
    redis::cmd("SET")
        .arg(format!("reader-session:{token}"))
        .arg(serde_json::to_string(&session)?)
        .arg("EX")
        .arg(604_800)
        .query_async::<()>(&mut redis)
        .await?;
    let cookie = Cookie::build(("oxide_reader", token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.session_secure)
        .path("/")
        .max_age(cookie::time::Duration::days(7))
        .build();
    let mut response = Json(ReaderSessionResponse {
        username: session.username,
        csrf_token: session.csrf_token,
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse()?);
    Ok(response)
}

/// 注册读者；付款权益不会随注册自动建立。
#[utoipa::path(post, path = "/api/v1/reader/register", request_body = ReaderCredentials, responses((status = 201, body = ReaderSessionResponse), (status = 400, body = ApiError), (status = 409, body = ApiError)), tag = "reader")]
pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<ReaderCredentials>,
) -> Response {
    if input.username.len() < 3
        || input.username.len() > 32
        || !input
            .username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || input.password.len() < 12
        || input.password.len() > 128
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_reader",
            "账号或密码格式无效",
        );
    }
    match allow_attempt(&state, &input.username).await {
        Ok(true) => {}
        Ok(false) => return error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "请稍后重试"),
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            );
        }
    }
    let exists = reader_user::Entity::find()
        .filter(reader_user::Column::Username.eq(&input.username))
        .one(&state.db)
        .await;
    match exists {
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "reader_exists", "账号已存在"),
        Err(err) => {
            tracing::error!(error = %err, "检查读者账号失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
        Ok(None) => {}
    }
    let password = input.password;
    let hash = tokio::task::spawn_blocking(move || -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        Ok(Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?
            .to_string())
    })
    .await;
    let Ok(Ok(hash)) = hash else {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "服务暂时不可用",
        );
    };
    let row = reader_user::ActiveModel {
        id: sea_orm::NotSet,
        public_id: Set(Uuid::new_v4()),
        username: Set(input.username),
        password_hash: Set(hash),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await;
    match row {
        Ok(row) => match create_session(&state, row).await {
            Ok(mut response) => {
                *response.status_mut() = StatusCode::CREATED;
                response
            }
            Err(err) => {
                tracing::error!(error = %err, "建立读者会话失败");
                error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "dependency_unavailable",
                    "服务暂时不可用",
                )
            }
        },
        Err(err) => {
            tracing::warn!(error = %err, "注册读者失败");
            error(StatusCode::CONFLICT, "reader_exists", "账号已存在")
        }
    }
}

/// 校验密码后创建读者会话。
#[utoipa::path(post, path = "/api/v1/reader/login", request_body = ReaderCredentials, responses((status = 200, body = ReaderSessionResponse), (status = 401, body = ApiError)), tag = "reader")]
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<ReaderCredentials>,
) -> Response {
    match allow_attempt(&state, &input.username).await {
        Ok(true) => {}
        Ok(false) => return error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "请稍后重试"),
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            );
        }
    }
    let row = match reader_user::Entity::find()
        .filter(reader_user::Column::Username.eq(input.username))
        .one(&state.db)
        .await
    {
        Ok(row) => row,
        Err(err) => {
            tracing::error!(error = %err, "读取读者失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let Some(row) = row else {
        return error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码错误",
        );
    };
    let password = input.password;
    let hash = row.password_hash.clone();
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).ok().is_some_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    if !valid {
        return error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码错误",
        );
    }
    match create_session(&state, row).await {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(error = %err, "建立读者会话失败");
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            )
        }
    }
}

/// 从 Cookie 读取会话，Redis 异常作为错误上抛，不能按已授权处理。
pub async fn get_session(state: &AppState, headers: &HeaderMap) -> Result<Option<ReaderSession>> {
    let token = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            Cookie::split_parse(cookies)
                .flatten()
                .find(|cookie| cookie.name() == "oxide_reader")
        })
        .map(|cookie| cookie.value().to_owned());
    let Some(token) = token else { return Ok(None) };
    let mut redis = state.redis.get_multiplexed_async_connection().await?;
    let value: Option<String> = redis::cmd("GET")
        .arg(format!("reader-session:{token}"))
        .query_async(&mut redis)
        .await?;
    value
        .map(|value| serde_json::from_str(&value).context("读者会话损坏"))
        .transpose()
}

/// 校验读者 Cookie 及写请求的 CSRF 令牌。
pub async fn require_reader(
    state: &AppState,
    headers: &HeaderMap,
    write: bool,
) -> std::result::Result<ReaderSession, Response> {
    let session = match get_session(state, headers).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            return Err(error(
                StatusCode::UNAUTHORIZED,
                "reader_required",
                "请先登录读者账号",
            ));
        }
        Err(err) => {
            tracing::error!(error = %err, "读取读者会话失败");
            return Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            ));
        }
    };
    if write
        && headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            != Some(session.csrf_token.as_str())
    {
        return Err(error(StatusCode::FORBIDDEN, "invalid_csrf", "请求校验失败"));
    }
    Ok(session)
}

/// 当前读者用于前端恢复登录状态。
#[utoipa::path(get, path = "/api/v1/reader/session", responses((status = 200, body = ReaderSessionResponse), (status = 401, body = ApiError)), tag = "reader")]
pub async fn current(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match require_reader(&state, &headers, false).await {
        Ok(session) => Json(ReaderSessionResponse {
            username: session.username,
            csrf_token: session.csrf_token,
        })
        .into_response(),
        Err(response) => response,
    }
}

/// 退出时删除会话并清除读者 Cookie。
#[utoipa::path(post, path = "/api/v1/reader/logout", responses((status = 200), (status = 401, body = ApiError)), tag = "reader")]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_reader(&state, &headers, true).await {
        return response;
    }
    if let Some(token) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            Cookie::split_parse(cookies)
                .flatten()
                .find(|cookie| cookie.name() == "oxide_reader")
        })
        .map(|cookie| cookie.value().to_owned())
        && let Ok(mut redis) = state.redis.get_multiplexed_async_connection().await
    {
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg(format!("reader-session:{token}"))
            .query_async(&mut redis)
            .await;
    }
    let cookie = Cookie::build(("oxide_reader", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.session_secure)
        .path("/")
        .max_age(cookie::time::Duration::ZERO)
        .build();
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Ok(value) = cookie.to_string().parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

/// 数据库中的已购记录是唯一授权依据。
pub async fn owns_column(state: &AppState, reader_id: i64, paid_column_id: i64) -> Result<bool> {
    Ok(column_subscription::Entity::find()
        .filter(column_subscription::Column::ReaderId.eq(reader_id))
        .filter(column_subscription::Column::PaidColumnId.eq(paid_column_id))
        .one(&state.db)
        .await?
        .is_some())
}
