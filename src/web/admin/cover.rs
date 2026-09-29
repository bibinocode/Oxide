//! 文章主图使用素材公开 UUID 关联，HTTP 层不输出内部主键。
use super::super::{ApiError, AppState, error};
use super::auth::require_admin;
use crate::entity::{article, asset, status::AssetVisibility};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// 发布面板选择的封面，null 表示移除。
#[derive(Deserialize, ToSchema)]
pub struct CoverInput {
    /// 素材公开 UUID。
    pub asset_public_id: Option<Uuid>,
}

/// 封面展示信息。
#[derive(Serialize, ToSchema)]
pub struct CoverResponse {
    /// 素材公开 UUID。
    pub asset_public_id: Option<Uuid>,
    /// 稳定媒体路径。
    pub media_url: Option<String>,
}

/// 读取文章的封面选择。
#[utoipa::path(get, path = "/api/v1/admin/articles/{public_id}/cover", params(("public_id" = Uuid, Path)), responses((status = 200, body = CoverResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let result = async {
        let row = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&state.db)
            .await?;
        let Some(row) = row else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let cover = match row.cover_asset_id {
            Some(id) => asset::Entity::find_by_id(id).one(&state.db).await?,
            None => None,
        };
        Ok(Some(CoverResponse {
            asset_public_id: cover.as_ref().map(|asset| asset.public_id),
            media_url: cover.map(|asset| format!("/media/{}", asset.public_id)),
        }))
    }
    .await;
    match result {
        Ok(Some(cover)) => Json(cover).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取封面失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "读取封面失败",
            )
        }
    }
}

/// 关联公开图片；私有素材必须先由管理员明确公开。
#[utoipa::path(put, path = "/api/v1/admin/articles/{public_id}/cover", params(("public_id" = Uuid, Path)), request_body = CoverInput, responses((status = 200, body = CoverResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<CoverInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let Some(row) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(error(
                StatusCode::NOT_FOUND,
                "article_not_found",
                "文章不存在",
            ));
        };
        let cover = match input.asset_public_id {
            Some(id) => match asset::Entity::find()
                .filter(asset::Column::PublicId.eq(id))
                .one(&state.db)
                .await?
            {
                Some(asset)
                    if asset.visibility == AssetVisibility::Public
                        && asset.mime_type.starts_with("image/") =>
                {
                    Some(asset)
                }
                _ => {
                    return Ok(error(
                        StatusCode::BAD_REQUEST,
                        "invalid_cover",
                        "请选择已公开的图片素材",
                    ));
                }
            },
            None => None,
        };
        let mut active = row.into_active_model();
        active.cover_asset_id = Set(cover.as_ref().map(|asset| asset.id));
        active.updated_at = Set(chrono::Utc::now());
        active.update(&state.db).await?;
        Ok(Json(CoverResponse {
            asset_public_id: cover.as_ref().map(|asset| asset.public_id),
            media_url: cover.map(|asset| format!("/media/{}", asset.public_id)),
        })
        .into_response())
    }
    .await;
    match result {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(error = %err, "保存封面失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "保存封面失败",
            )
        }
    }
}
