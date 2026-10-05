//! 管理员维护站点信息、分类和标签。

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::entity::{asset, category, site_setting, status::AssetVisibility, tag};

use super::super::{
    ApiError, AppState,
    client::content::{SiteResponse, TaxonomyResponse},
    error,
};
use super::auth::require_admin;

/// 可编辑的公开站点信息。
#[derive(Deserialize, ToSchema)]
pub struct SiteInput {
    /// 站点名称。
    pub site_name: String,
    /// 可选简介。
    pub description: Option<String>,
    /// 站点绝对 URL，用于 RSS 链接。
    pub base_url: String,
    /// 公开模块配置；省略时保留已有配置。
    pub presentation: Option<crate::domain::site::SitePresentation>,
}

/// 分类与标签写入请求。
#[derive(Deserialize, ToSchema)]
pub struct TaxonomyInput {
    /// 展示名称。
    pub name: String,
    /// URL slug。
    pub slug: String,
}

/// 验证分类与标签的可公开字段。
fn valid_taxonomy(input: &TaxonomyInput) -> bool {
    let slug = input.slug.as_bytes();
    !input.name.trim().is_empty()
        && input.name.chars().count() <= 60
        && (2..=80).contains(&slug.len())
        && slug.first() != Some(&b'-')
        && slug.last() != Some(&b'-')
        && slug
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// 保存站点名称、描述与公开地址；缺失时创建固定 ID 的设置行。
#[utoipa::path(put, path = "/api/v1/admin/site", request_body = SiteInput, responses((status = 200, body = SiteResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn update_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SiteInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let parsed = url::Url::parse(&input.base_url);
    if input
        .presentation
        .as_ref()
        .is_some_and(|value| !value.is_valid())
        || input.site_name.trim().is_empty()
        || input.site_name.chars().count() > 80
        || input
            .description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 500)
        || !parsed
            .as_ref()
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
    {
        return error(StatusCode::BAD_REQUEST, "invalid_site", "站点设置无效");
    }
    if let Some(portrait_url) = input
        .presentation
        .as_ref()
        .map(|value| &value.home_intro.portrait_url)
        && !portrait_url.is_empty()
    {
        let id = uuid::Uuid::parse_str(portrait_url.trim_start_matches("/media/"))
            .expect("公开配置已验证素材地址");
        match asset::Entity::find()
            .filter(asset::Column::PublicId.eq(id))
            .one(&state.db)
            .await
        {
            Ok(Some(row))
                if row.visibility == AssetVisibility::Public
                    && row.mime_type.starts_with("image/") => {}
            Ok(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "invalid_portrait",
                    "请选择已公开的图片素材",
                );
            }
            Err(err) => {
                tracing::error!(error = %err, "校验首页肖像素材失败");
                return error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "服务暂时不可用",
                );
            }
        }
    }
    let previous = match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(row) => row,
        Err(err) => {
            tracing::error!(error = %err, "读取站点设置失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let active_provider_id = previous
        .as_ref()
        .and_then(|row| row.active_provider_id.clone());
    let active = site_setting::ActiveModel {
        id: Set(1),
        site_name: Set(input.site_name.trim().into()),
        description: Set(input.description),
        base_url: Set(input.base_url.trim_end_matches('/').into()),
        presentation: Set(input
            .presentation
            .map(|value| serde_json::to_value(value).expect("公开配置可序列化"))
            .or_else(|| previous.as_ref().and_then(|row| row.presentation.clone()))),
        active_provider_id: Set(active_provider_id),
        updated_at: Set(chrono::Utc::now()),
    };
    let result = if previous.is_some() {
        active.update(&state.db).await
    } else {
        active.insert(&state.db).await
    };
    match result {
        Ok(row) => Json(SiteResponse {
            site_name: row.site_name,
            description: row.description,
            base_url: row.base_url,
            presentation: row
                .presentation
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default(),
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "保存站点设置失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 新建分类。
#[utoipa::path(post, path = "/api/v1/admin/categories", request_body = TaxonomyInput, responses((status = 201, body = TaxonomyResponse), (status = 400, body = ApiError), (status = 409, body = ApiError)), tag = "admin")]
pub async fn create_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TaxonomyInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid_taxonomy(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_taxonomy", "分类内容无效");
    }
    match category::Entity::find()
        .filter(category::Column::Slug.eq(&input.slug))
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "slug_conflict", "slug 已存在"),
        Err(err) => {
            tracing::error!(error = %err, "检查分类失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
        _ => {}
    }
    match (category::ActiveModel {
        visible: Set(true),
        name: Set(input.name.trim().into()),
        slug: Set(input.slug),
        ..Default::default()
    })
    .insert(&state.db)
    .await
    {
        Ok(row) => (
            StatusCode::CREATED,
            Json(TaxonomyResponse {
                name: row.name,
                slug: row.slug,
                visible: row.visible,
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "创建分类失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 更新分类名称或 slug。
#[utoipa::path(put, path = "/api/v1/admin/categories/{slug}", params(("slug" = String, Path)), request_body = TaxonomyInput, responses((status = 200, body = TaxonomyResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn update_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(input): Json<TaxonomyInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid_taxonomy(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_taxonomy", "分类内容无效");
    }
    let row = match category::Entity::find()
        .filter(category::Column::Slug.eq(slug))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "category_not_found", "分类不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取分类失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut active = row.into_active_model();
    active.name = Set(input.name.trim().into());
    active.slug = Set(input.slug);
    match active.update(&state.db).await {
        Ok(row) => Json(TaxonomyResponse {
            name: row.name,
            slug: row.slug,
            visible: row.visible,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "更新分类失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除分类并移除文章关联；正文保留，同一事务保证一致性。
#[utoipa::path(delete, path = "/api/v1/admin/categories/{slug}", params(("slug" = String, Path)), responses((status = 204), (status = 409, body = ApiError)), tag = "admin")]
pub async fn delete_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = category::Entity::find()
            .filter(category::Column::Slug.eq(slug))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(false);
        };
        // 只清除分类/标签关联，绝不删除其下文章。
        crate::entity::article_category::Entity::delete_many()
            .filter(crate::entity::article_category::Column::CategoryId.eq(row.id))
            .exec(&txn)
            .await?;
        category::Entity::delete_by_id(row.id).exec(&txn).await?;
        txn.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "category_not_found", "条目不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "删除分类标签失败",
        ),
    }
}

/// 新建标签。
#[utoipa::path(post, path = "/api/v1/admin/tags", request_body = TaxonomyInput, responses((status = 201, body = TaxonomyResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn create_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TaxonomyInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid_taxonomy(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_taxonomy", "标签内容无效");
    }
    match tag::Entity::find()
        .filter(tag::Column::Slug.eq(&input.slug))
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "slug_conflict", "slug 已存在"),
        Err(err) => {
            tracing::error!(error = %err, "检查标签失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
        _ => {}
    }
    match (tag::ActiveModel {
        visible: Set(true),
        name: Set(input.name.trim().into()),
        slug: Set(input.slug),
        ..Default::default()
    })
    .insert(&state.db)
    .await
    {
        Ok(row) => (
            StatusCode::CREATED,
            Json(TaxonomyResponse {
                name: row.name,
                slug: row.slug,
                visible: row.visible,
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "创建标签失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 更新标签名称和 slug。
#[utoipa::path(put, path = "/api/v1/admin/tags/{slug}", params(("slug" = String, Path)), request_body = TaxonomyInput, responses((status = 200, body = TaxonomyResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn update_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(input): Json<TaxonomyInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid_taxonomy(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_taxonomy", "标签内容无效");
    }
    let row = match tag::Entity::find()
        .filter(tag::Column::Slug.eq(slug))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "tag_not_found", "标签不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取标签失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut active = row.into_active_model();
    active.name = Set(input.name.trim().into());
    active.slug = Set(input.slug);
    match active.update(&state.db).await {
        Ok(row) => Json(TaxonomyResponse {
            name: row.name,
            slug: row.slug,
            visible: row.visible,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "更新标签失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除标签并移除文章关联，不删除文章正文。
#[utoipa::path(delete, path = "/api/v1/admin/tags/{slug}", params(("slug" = String, Path)), responses((status = 204), (status = 409, body = ApiError)), tag = "admin")]
pub async fn delete_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = tag::Entity::find()
            .filter(tag::Column::Slug.eq(slug))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(false);
        };
        // 只清除分类/标签关联，绝不删除其下文章。
        crate::entity::article_tag::Entity::delete_many()
            .filter(crate::entity::article_tag::Column::TagId.eq(row.id))
            .exec(&txn)
            .await?;
        tag::Entity::delete_by_id(row.id).exec(&txn).await?;
        txn.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "tag_not_found", "条目不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "删除分类标签失败",
        ),
    }
}

/// 管理端列出全部 category，包含隐藏条目。
#[utoipa::path(get, path = "/api/v1/admin/categories", responses((status = 200, body = Vec<TaxonomyResponse>)), tag = "admin")]
pub async fn list_category(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match category::Entity::find()
        .order_by_asc(category::Column::Name)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(
            rows.into_iter()
                .map(|row| TaxonomyResponse {
                    name: row.name,
                    slug: row.slug,
                    visible: row.visible,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "分类标签读取失败",
        ),
    }
}
/// 隐藏分类标签不改变文章发布状态或所属关系。
#[utoipa::path(patch, path = "/api/v1/admin/categories/{slug}", params(("slug" = String, Path)), request_body = super::ContentVisibilityInput, responses((status = 200, body = TaxonomyResponse)), tag = "admin")]
pub async fn visibility_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(input): Json<super::ContentVisibilityInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let Some(row) = category::Entity::find()
            .filter(category::Column::Slug.eq(slug))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let mut active = row.into_active_model();
        active.visible = Set(input.visible);
        active.update(&state.db).await.map(Some)
    }
    .await;
    match result {
        Ok(Some(row)) => Json(TaxonomyResponse {
            name: row.name,
            slug: row.slug,
            visible: row.visible,
        })
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "category_not_found", "条目不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "可见性修改失败",
        ),
    }
}

/// 管理端列出全部 tag，包含隐藏条目。
#[utoipa::path(get, path = "/api/v1/admin/tags", responses((status = 200, body = Vec<TaxonomyResponse>)), tag = "admin")]
pub async fn list_tag(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match tag::Entity::find()
        .order_by_asc(tag::Column::Name)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(
            rows.into_iter()
                .map(|row| TaxonomyResponse {
                    name: row.name,
                    slug: row.slug,
                    visible: row.visible,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "分类标签读取失败",
        ),
    }
}
/// 隐藏分类标签不改变文章发布状态或所属关系。
#[utoipa::path(patch, path = "/api/v1/admin/tags/{slug}", params(("slug" = String, Path)), request_body = super::ContentVisibilityInput, responses((status = 200, body = TaxonomyResponse)), tag = "admin")]
pub async fn visibility_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(input): Json<super::ContentVisibilityInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let Some(row) = tag::Entity::find()
            .filter(tag::Column::Slug.eq(slug))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let mut active = row.into_active_model();
        active.visible = Set(input.visible);
        active.update(&state.db).await.map(Some)
    }
    .await;
    match result {
        Ok(Some(row)) => Json(TaxonomyResponse {
            name: row.name,
            slug: row.slug,
            visible: row.visible,
        })
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "tag_not_found", "条目不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "可见性修改失败",
        ),
    }
}
