//! 稳定素材地址及私有素材访问控制。

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use object_store::{ObjectStoreExt, path::Path as ObjectPath};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    entity::{asset, status::AssetVisibility, storage_provider},
    infrastructure::storage::client,
};

use super::super::admin::auth::require_admin;
use super::super::{ApiError, AppState, error};

/// 稳定媒体地址从素材创建时的提供商读取对象，不受后续切换影响。
#[utoipa::path(get, path = "/media/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 200, content_type = "image/*"), (status = 404, body = ApiError)), tag = "site")]
pub async fn media(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    let row = match asset::Entity::find()
        .filter(asset::Column::PublicId.eq(public_id))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "asset_not_found", "素材不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取媒体元数据失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if row.visibility == AssetVisibility::Private
        && require_admin(&state, &headers, false).await.is_err()
    {
        return error(StatusCode::NOT_FOUND, "asset_not_found", "素材不存在");
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
    let store = match client(&provider, &state.comment_hash_key) {
        Ok(store) => store,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "provider_unconfigured",
                "存储提供商配置不完整",
            );
        }
    };
    let bytes = match store.get(&ObjectPath::from(row.object_key)).await {
        Ok(object) => match object.bytes().await {
            Ok(bytes) => bytes,
            Err(_) => {
                return error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "storage_unavailable",
                    "存储暂时不可用",
                );
            }
        },
        Err(_) => return error(StatusCode::NOT_FOUND, "asset_not_found", "素材不存在"),
    };
    let cache = if row.visibility == AssetVisibility::Public {
        "public, max-age=86400"
    } else {
        "private, no-store"
    };
    let mut response = bytes.into_response();
    if let Ok(mime) = row.mime_type.parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, mime);
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        cache.parse().expect("静态缓存标头有效"),
    );
    response
}
