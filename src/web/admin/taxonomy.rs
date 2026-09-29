//! 文章与分类、标签的关联管理。

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::{article, article_category, article_tag, category, tag};

use super::super::{ApiError, AppState, error};
use super::auth::require_admin;

/// 文章所属分类和标签的 slug 列表。
#[derive(Deserialize, Serialize, ToSchema)]
pub struct ArticleTaxonomy {
    /// 分类 slug。
    pub categories: Vec<String>,
    /// 标签 slug。
    pub tags: Vec<String>,
}

/// 读取文章关联，永不输出数据库内部 ID。
#[utoipa::path(get, path = "/api/v1/admin/articles/{public_id}/taxonomy", params(("public_id" = Uuid, Path)), responses((status = 200, body = ArticleTaxonomy), (status = 404, body = ApiError)), tag = "admin")]
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let result = async {
        let Some(row) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let categories = article_category::Entity::find()
            .filter(article_category::Column::ArticleId.eq(row.id))
            .all(&state.db)
            .await?;
        let tags = article_tag::Entity::find()
            .filter(article_tag::Column::ArticleId.eq(row.id))
            .all(&state.db)
            .await?;
        let category_rows = category::Entity::find()
            .filter(
                category::Column::Id.is_in(
                    categories
                        .into_iter()
                        .map(|row| row.category_id)
                        .collect::<Vec<_>>(),
                ),
            )
            .all(&state.db)
            .await?;
        let tag_rows = tag::Entity::find()
            .filter(
                tag::Column::Id.is_in(tags.into_iter().map(|row| row.tag_id).collect::<Vec<_>>()),
            )
            .all(&state.db)
            .await?;
        Ok(Some(ArticleTaxonomy {
            categories: category_rows.into_iter().map(|row| row.slug).collect(),
            tags: tag_rows.into_iter().map(|row| row.slug).collect(),
        }))
    }
    .await;
    match result {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取文章分类标签失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 在事务内整体替换关联；输入中的 slug 必须全部存在。
#[utoipa::path(put, path = "/api/v1/admin/articles/{public_id}/taxonomy", params(("public_id" = Uuid, Path)), request_body = ArticleTaxonomy, responses((status = 200, body = ArticleTaxonomy), (status = 400, body = ApiError), (status = 404, body = ApiError)), tag = "admin")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<ArticleTaxonomy>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if input.categories.len() > 20 || input.tags.len() > 30 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_taxonomy",
            "分类或标签数量过多",
        );
    }
    let categories = match category::Entity::find()
        .filter(category::Column::Slug.is_in(input.categories.clone()))
        .all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "读取分类失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let tags = match tag::Entity::find()
        .filter(tag::Column::Slug.is_in(input.tags.clone()))
        .all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "读取标签失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if categories.len() != input.categories.len() || tags.len() != input.tags.len() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_taxonomy",
            "分类或标签不存在，或包含重复项",
        );
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(article) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(false);
        };
        article_category::Entity::delete_many()
            .filter(article_category::Column::ArticleId.eq(article.id))
            .exec(&txn)
            .await?;
        article_tag::Entity::delete_many()
            .filter(article_tag::Column::ArticleId.eq(article.id))
            .exec(&txn)
            .await?;
        for category in categories {
            article_category::ActiveModel {
                article_id: Set(article.id),
                category_id: Set(category.id),
            }
            .insert(&txn)
            .await?;
        }
        for tag in tags {
            article_tag::ActiveModel {
                article_id: Set(article.id),
                tag_id: Set(tag.id),
            }
            .insert(&txn)
            .await?;
        }
        txn.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => Json(input).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "保存文章分类标签失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}
