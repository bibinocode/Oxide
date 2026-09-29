//! 存储提供商切换、素材上传及稳定媒体地址。

use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::site::SitePresentation,
    entity::{
        asset, site_setting,
        status::{AssetVisibility, StorageProviderKind},
        storage_provider,
    },
    infrastructure::storage::{self, Credentials, encrypt_credentials},
};

use super::super::{ApiError, AppState, error};
use super::auth::require_admin;

/// 阿里云 OSS 或七牛 Kodo 的非敏感配置。
#[derive(Deserialize, ToSchema)]
pub struct ProviderInput {
    /// 稳定 ID，供环境变量命名及旧素材引用。
    pub id: String,
    /// `aliyun_oss` 或 `qiniu_kodo`。
    pub kind: String,
    /// 管理端显示名称。
    pub name: String,
    /// 公开 CDN 地址。
    pub public_base_url: String,
    /// S3 兼容端点；七牛可填写所属区域的 s3 域名。
    pub endpoint: Option<String>,
    /// 空间名称。
    pub bucket: Option<String>,
    /// 区域标识。
    pub region: Option<String>,
    /// 新建时填写；更新时留空表示保留旧值。
    pub access_key: Option<String>,
    /// 新建时填写；更新时留空表示保留旧值。
    pub secret_key: Option<String>,
    /// 是否接受新上传。
    pub upload_enabled: bool,
}

/// 提供商公开元数据，不返回凭据。
#[derive(Serialize, ToSchema)]
pub struct ProviderResponse {
    /// 稳定 ID。
    pub id: String,
    /// 存储类型。
    pub kind: &'static str,
    /// 显示名称。
    pub name: String,
    /// CDN 地址。
    pub public_base_url: String,
    /// 当前 S3 端点。
    pub endpoint: Option<String>,
    /// 空间名称。
    pub bucket: Option<String>,
    /// 区域标识。
    pub region: Option<String>,
    /// 是否已保存加密凭据；旧环境变量无法从这里探测。
    pub credentials_configured: bool,
    /// 上传开关。
    pub upload_enabled: bool,
    /// 是否为当前新上传提供商。
    pub active: bool,
}

/// 素材元数据，链接始终使用本站稳定 UUID 地址。
#[derive(Serialize, ToSchema)]
pub struct AssetResponse {
    /// 素材 UUID。
    pub public_id: Uuid,
    /// 稳定媒体地址。
    pub media_url: String,
    /// MIME 类型。
    pub mime_type: String,
    /// 字节大小。
    pub size_bytes: i64,
    /// 像素宽度。
    pub width: Option<i32>,
    /// 像素高度。
    pub height: Option<i32>,
    /// 公开或私有。
    pub visibility: &'static str,
    /// 上传时间。
    pub created_at: DateTime<Utc>,
}

/// 更改素材可见性。
#[derive(Deserialize, ToSchema)]
pub struct VisibilityInput {
    /// 公开后访客可通过稳定媒体 URL 访问。
    pub public: bool,
}

/// 将数据库枚举映射为稳定的 API 字符串。
fn kind_name(kind: StorageProviderKind) -> &'static str {
    match kind {
        StorageProviderKind::AliyunOss => "aliyun_oss",
        StorageProviderKind::QiniuKodo => "qiniu_kodo",
    }
}

/// 输入校验同时限制动态环境变量名，避免不同 ID 产生键名冲突。
fn validate_provider(input: &ProviderInput) -> Option<StorageProviderKind> {
    if input.id.len() < 2
        || input.id.len() > 40
        || !input
            .id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        || input.name.trim().is_empty()
        || input.name.chars().count() > 80
        || !url::Url::parse(&input.public_base_url)
            .ok()
            .is_some_and(|url| url.scheme() == "https" && url.host_str().is_some())
    {
        return None;
    }
    if let Some(endpoint) = &input.endpoint
        && !url::Url::parse(endpoint).ok().is_some_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.path() == "/"
                && url.query().is_none()
        })
    {
        return None;
    }
    if input
        .bucket
        .as_ref()
        .is_some_and(|v| v.trim().is_empty() || v.len() > 120)
        || input
            .region
            .as_ref()
            .is_some_and(|v| v.trim().is_empty() || v.len() > 120)
        || input.access_key.as_ref().is_some_and(|v| v.len() > 256)
        || input.secret_key.as_ref().is_some_and(|v| v.len() > 256)
    {
        return None;
    }
    match input.kind.as_str() {
        "aliyun_oss" => Some(StorageProviderKind::AliyunOss),
        "qiniu_kodo" => Some(StorageProviderKind::QiniuKodo),
        _ => None,
    }
}

/// 获取站点当前上传提供商 ID。
async fn active_provider_id(state: &AppState) -> Result<Option<String>, sea_orm::DbErr> {
    Ok(site_setting::Entity::find_by_id(1)
        .one(&state.db)
        .await?
        .and_then(|settings| settings.active_provider_id))
}

/// 列出非敏感的提供商设置。
#[utoipa::path(get, path = "/api/v1/admin/storage-providers", responses((status = 200, body = Vec<ProviderResponse>), (status = 401, body = ApiError)), tag = "admin")]
pub async fn providers(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let result = async {
        let rows = storage_provider::Entity::find()
            .order_by_asc(storage_provider::Column::Name)
            .all(&state.db)
            .await?;
        Ok::<_, sea_orm::DbErr>((rows, active_provider_id(&state).await?))
    }
    .await;
    match result {
        Ok((rows, active)) => Json(
            rows.into_iter()
                .map(|row| ProviderResponse {
                    active: active.as_deref() == Some(row.id.as_str()),
                    id: row.id,
                    kind: kind_name(row.kind),
                    name: row.name,
                    public_base_url: row.public_base_url,
                    endpoint: row.endpoint,
                    bucket: row.bucket,
                    region: row.region,
                    credentials_configured: row.encrypted_credentials.is_some(),
                    upload_enabled: row.upload_enabled,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取存储提供商失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 创建提供商；连接凭据在服务端加密保存。
#[utoipa::path(post, path = "/api/v1/admin/storage-providers", request_body = ProviderInput, responses((status = 201, body = ProviderResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProviderInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let Some(kind) = validate_provider(&input) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_provider",
            "存储提供商配置无效",
        );
    };
    let encrypted_credentials = match (&input.access_key, &input.secret_key) {
        (Some(access_key), Some(secret_key))
            if !access_key.is_empty() && !secret_key.is_empty() =>
        {
            match encrypt_credentials(
                &Credentials {
                    access_key: access_key.clone(),
                    secret_key: secret_key.clone(),
                },
                &state.comment_hash_key,
            ) {
                Ok(value) => Some(value),
                Err(_) => {
                    return error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "encryption_failed",
                        "保存凭据失败",
                    );
                }
            }
        }
        (None, None) => None,
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_credentials",
                "请同时填写 Access Key 和 Secret Key",
            );
        }
    };
    let model = storage_provider::ActiveModel {
        id: Set(input.id),
        kind: Set(kind),
        name: Set(input.name.trim().into()),
        public_base_url: Set(input.public_base_url.trim_end_matches('/').into()),
        endpoint: Set(input.endpoint),
        bucket: Set(input.bucket),
        region: Set(input.region),
        encrypted_credentials: Set(encrypted_credentials),
        upload_enabled: Set(input.upload_enabled),
        created_at: Set(Utc::now()),
    };
    match model.insert(&state.db).await {
        Ok(row) => (
            StatusCode::CREATED,
            Json(ProviderResponse {
                id: row.id,
                kind: kind_name(row.kind),
                name: row.name,
                public_base_url: row.public_base_url,
                endpoint: row.endpoint,
                bucket: row.bucket,
                region: row.region,
                credentials_configured: row.encrypted_credentials.is_some(),
                upload_enabled: row.upload_enabled,
                active: false,
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "创建存储提供商失败");
            error(
                StatusCode::CONFLICT,
                "provider_conflict",
                "存储提供商已存在",
            )
        }
    }
}

/// 更新名称、域名和上传开关；不能改变 ID 或已有素材的提供商类型。
#[utoipa::path(put, path = "/api/v1/admin/storage-providers/{id}", params(("id" = String, Path)), request_body = ProviderInput, responses((status = 200, body = ProviderResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn update_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let Some(kind) = validate_provider(&input) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_provider",
            "存储提供商配置无效",
        );
    };
    if id != input.id {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_provider",
            "提供商 ID 不可修改",
        );
    }
    let row = match storage_provider::Entity::find_by_id(id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "provider_not_found", "提供商不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取提供商失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if kind != row.kind {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_provider",
            "提供商类型不可修改",
        );
    }
    let mut active = row.into_active_model();
    active.name = Set(input.name.trim().into());
    active.public_base_url = Set(input.public_base_url.trim_end_matches('/').into());
    active.endpoint = Set(input.endpoint);
    active.bucket = Set(input.bucket);
    active.region = Set(input.region);
    match (&input.access_key, &input.secret_key) {
        (Some(access_key), Some(secret_key))
            if !access_key.is_empty() && !secret_key.is_empty() =>
        {
            match encrypt_credentials(
                &Credentials {
                    access_key: access_key.clone(),
                    secret_key: secret_key.clone(),
                },
                &state.comment_hash_key,
            ) {
                Ok(value) => active.encrypted_credentials = Set(Some(value)),
                Err(_) => {
                    return error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "encryption_failed",
                        "保存凭据失败",
                    );
                }
            }
        }
        (None, None) => {}
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_credentials",
                "请同时填写 Access Key 和 Secret Key",
            );
        }
    }
    active.upload_enabled = Set(input.upload_enabled);
    match active.update(&state.db).await {
        Ok(row) => Json(ProviderResponse {
            id: row.id.clone(),
            kind: kind_name(row.kind),
            name: row.name,
            public_base_url: row.public_base_url,
            endpoint: row.endpoint,
            bucket: row.bucket,
            region: row.region,
            credentials_configured: row.encrypted_credentials.is_some(),
            upload_enabled: row.upload_enabled,
            active: active_provider_id(&state).await.ok().flatten().as_deref()
                == Some(row.id.as_str()),
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "更新提供商失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 切换当前上传提供商，仅影响后续素材。
#[utoipa::path(post, path = "/api/v1/admin/storage-providers/{id}/activate", params(("id" = String, Path)), responses((status = 200, body = ProviderResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn activate_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let row = match storage_provider::Entity::find_by_id(&id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) if row.upload_enabled => row,
        Ok(Some(_)) => {
            return error(
                StatusCode::CONFLICT,
                "provider_disabled",
                "提供商未启用上传",
            );
        }
        Ok(None) => return error(StatusCode::NOT_FOUND, "provider_not_found", "提供商不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取提供商失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if storage::configured(&row, &state.comment_hash_key).is_err() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_unconfigured",
            "存储提供商连接配置不完整",
        );
    }
    let previous = match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(value) => value,
        Err(err) => {
            tracing::error!(error = %err, "读取站点配置失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let model = site_setting::ActiveModel {
        id: Set(1),
        site_name: Set(previous
            .as_ref()
            .map_or("Oxide".into(), |settings| settings.site_name.clone())),
        description: Set(previous
            .as_ref()
            .and_then(|settings| settings.description.clone())),
        base_url: Set(previous
            .as_ref()
            .map_or(state.public_base_url.to_string(), |settings| {
                settings.base_url.clone()
            })),
        active_provider_id: Set(Some(id)),
        presentation: Set(previous
            .as_ref()
            .and_then(|settings| settings.presentation.clone())),
        updated_at: Set(Utc::now()),
    };
    let result = if previous.is_some() {
        model.update(&state.db).await
    } else {
        model.insert(&state.db).await
    };
    match result {
        Ok(_) => Json(ProviderResponse {
            id: row.id,
            kind: kind_name(row.kind),
            name: row.name,
            public_base_url: row.public_base_url,
            endpoint: row.endpoint,
            bucket: row.bucket,
            region: row.region,
            credentials_configured: row.encrypted_credentials.is_some(),
            upload_enabled: row.upload_enabled,
            active: true,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "切换提供商失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 无旧素材引用时删除提供商；当前上传源会在同一事务中清空。
#[utoipa::path(delete, path = "/api/v1/admin/storage-providers/{id}", params(("id" = String, Path)), responses((status = 204), (status = 409, body = ApiError)), tag = "admin")]
pub async fn delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        if storage_provider::Entity::find_by_id(&id)
            .one(&txn)
            .await?
            .is_none()
        {
            return Ok::<_, sea_orm::DbErr>(None);
        }
        if asset::Entity::find()
            .filter(asset::Column::ProviderId.eq(&id))
            .one(&txn)
            .await?
            .is_some()
        {
            return Ok(Some(false));
        }
        if let Some(settings) = site_setting::Entity::find_by_id(1).one(&txn).await?
            && settings.active_provider_id.as_deref() == Some(id.as_str())
        {
            let mut active = settings.into_active_model();
            active.active_provider_id = Set(None);
            active.updated_at = Set(Utc::now());
            active.update(&txn).await?;
        }
        storage_provider::Entity::delete_by_id(&id)
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(Some(true))
    }
    .await;
    match result {
        Ok(Some(true)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Some(false)) => error(
            StatusCode::CONFLICT,
            "provider_in_use",
            "提供商仍有素材引用，请先处理素材",
        ),
        Ok(None) => error(StatusCode::NOT_FOUND, "provider_not_found", "提供商不存在"),
        Err(err) => {
            tracing::error!(error = %err, "删除存储提供商失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 将数据库素材映射为稳定公开标识。
fn asset_response(row: asset::Model) -> AssetResponse {
    AssetResponse {
        public_id: row.public_id,
        media_url: format!("/media/{}", row.public_id),
        mime_type: row.mime_type,
        size_bytes: row.size_bytes,
        width: row.width,
        height: row.height,
        visibility: if row.visibility == AssetVisibility::Public {
            "public"
        } else {
            "private"
        },
        created_at: row.created_at,
    }
}

/// 管理员素材库默认展示最近 100 项。
#[utoipa::path(get, path = "/api/v1/admin/assets", responses((status = 200, body = Vec<AssetResponse>), (status = 401, body = ApiError)), tag = "admin")]
pub async fn assets(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match asset::Entity::find()
        .order_by_desc(asset::Column::CreatedAt)
        .limit(100)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(rows.into_iter().map(asset_response).collect::<Vec<_>>()).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取素材失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 上传经文件头和尺寸验证的图片，初始为私有素材。
#[utoipa::path(post, path = "/api/v1/admin/assets", request_body(content = String, content_type = "multipart/form-data"), responses((status = 201, body = AssetResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    tracing::debug!("开始处理素材上传");
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let provider_id = match active_provider_id(&state).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return error(
                StatusCode::CONFLICT,
                "provider_missing",
                "尚未选择上传提供商",
            );
        }
        Err(err) => {
            tracing::error!(error = %err, "读取当前提供商失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let provider = match storage_provider::Entity::find_by_id(&provider_id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) if row.upload_enabled => row,
        _ => {
            return error(
                StatusCode::CONFLICT,
                "provider_disabled",
                "当前提供商不可上传",
            );
        }
    };
    let field = match multipart.next_field().await {
        Ok(Some(field)) if field.name() == Some("file") => field,
        _ => return error(StatusCode::BAD_REQUEST, "invalid_asset", "请选择图片文件"),
    };
    let bytes = match field.bytes().await {
        Ok(bytes) => bytes,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_asset", "无法读取图片"),
    };
    tracing::debug!("已读取上传数据");
    if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
        return error(StatusCode::BAD_REQUEST, "invalid_asset", "图片大小无效");
    }
    let Some(kind) = infer::get(&bytes) else {
        return error(StatusCode::BAD_REQUEST, "invalid_asset", "图片格式无效");
    };
    let (mime, extension) = match kind.mime_type() {
        "image/png" => ("image/png", "png"),
        "image/jpeg" => ("image/jpeg", "jpg"),
        "image/gif" => ("image/gif", "gif"),
        "image/webp" => ("image/webp", "webp"),
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_asset",
                "仅支持 PNG、JPEG、GIF 和 WebP",
            );
        }
    };
    let dimensions = match imagesize::blob_size(&bytes) {
        Ok(size) if size.width <= 10_000 && size.height <= 10_000 => size,
        _ => return error(StatusCode::BAD_REQUEST, "invalid_asset", "图片尺寸无效"),
    };
    tracing::debug!("图片格式与尺寸校验完成");
    let public_id = Uuid::new_v4();
    let key = format!("assets/{public_id}.{extension}");
    if let Err(err) = storage::configured(&provider, &state.comment_hash_key) {
        tracing::error!(error = %err, "初始化存储失败");
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_unconfigured",
            "存储提供商配置不完整",
        );
    }
    tracing::debug!(provider = %provider.id, "开始写入对象存储");
    if let Err(err) = storage::put(
        &provider,
        &state.comment_hash_key,
        &key,
        bytes.clone(),
        mime,
    )
    .await
    {
        tracing::error!(error = %err, "上传素材失败");
        let message = match provider.kind {
            StorageProviderKind::QiniuKodo => {
                "七牛上传失败，请检查空间写入权限、绑定域名及网络连接"
            }
            StorageProviderKind::AliyunOss => "阿里云 OSS 上传失败，请检查端点、区域和写入权限",
        };
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            message,
        );
    }
    tracing::debug!("对象存储写入完成");
    let model = asset::ActiveModel {
        public_id: Set(public_id),
        provider_id: Set(provider.id.clone()),
        object_key: Set(key.clone()),
        mime_type: Set(mime.into()),
        size_bytes: Set(bytes.len() as i64),
        width: Set(Some(dimensions.width as i32)),
        height: Set(Some(dimensions.height as i32)),
        visibility: Set(AssetVisibility::Private),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    match model.insert(&state.db).await {
        Ok(row) => (StatusCode::CREATED, Json(asset_response(row))).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "保存素材元数据失败");
            let _ = storage::delete(&provider, &state.comment_hash_key, &key).await;
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 管理员决定素材是否可公开访问。
#[utoipa::path(patch, path = "/api/v1/admin/assets/{public_id}", params(("public_id" = Uuid, Path)), request_body = VisibilityInput, responses((status = 200, body = AssetResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn set_visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<VisibilityInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let row = match asset::Entity::find()
        .filter(asset::Column::PublicId.eq(public_id))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "asset_not_found", "素材不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取素材失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if !input.public && portrait_uses_asset(&state, public_id).await {
        return error(
            StatusCode::CONFLICT,
            "asset_in_use",
            "首页肖像正在使用该素材",
        );
    }
    let mut active = row.into_active_model();
    active.visibility = Set(if input.public {
        AssetVisibility::Public
    } else {
        AssetVisibility::Private
    });
    match active.update(&state.db).await {
        Ok(row) => Json(asset_response(row)).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "更新素材可见性失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除素材记录后清理对象；被文章引用的素材无法删除。
#[utoipa::path(delete, path = "/api/v1/admin/assets/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 204), (status = 404, body = ApiError)), tag = "admin")]
pub async fn delete_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let row = match asset::Entity::find()
        .filter(asset::Column::PublicId.eq(public_id))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "asset_not_found", "素材不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取素材失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if portrait_uses_asset(&state, public_id).await {
        return error(
            StatusCode::CONFLICT,
            "asset_in_use",
            "首页肖像正在使用该素材",
        );
    }
    let provider = match storage_provider::Entity::find_by_id(&row.provider_id)
        .one(&state.db)
        .await
    {
        Ok(Some(provider)) => provider,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "存储暂时不可用",
            );
        }
    };
    if storage::configured(&provider, &state.comment_hash_key).is_err() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_unconfigured",
            "存储提供商配置不完整",
        );
    }
    if asset::Entity::delete_by_id(row.id)
        .exec(&state.db)
        .await
        .is_err()
    {
        return error(StatusCode::CONFLICT, "asset_in_use", "素材仍被文章使用");
    }
    if let Err(err) = storage::delete(&provider, &state.comment_hash_key, &row.object_key).await {
        tracing::error!(error = %err, "清理对象存储素材失败");
    }
    StatusCode::NO_CONTENT.into_response()
}

/// 检查首页配置对素材的引用，避免删除或私有化后出现失效肖像。
async fn portrait_uses_asset(state: &AppState, public_id: Uuid) -> bool {
    match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(Some(row)) => row
            .presentation
            .and_then(|value| serde_json::from_value::<SitePresentation>(value).ok())
            .is_some_and(|value| value.home_intro.portrait_url == format!("/media/{public_id}")),
        Ok(None) => false,
        Err(err) => {
            tracing::error!(error = %err, "检查首页肖像素材引用失败");
            true
        }
    }
}
