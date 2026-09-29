//! 公开分词搜索接口。

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::entity::{article, status::ArticleStatus};

use super::super::{ApiError, AppState, ArticlePageResponse, ArticleSummaryResponse, error};

/// 搜索词和有界分页参数。
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// 中文或英文检索词。
    pub q: String,
    /// 从 1 开始的页码。
    pub page: Option<u64>,
    /// 每页条数，最多 50。
    pub per_page: Option<u64>,
}

/// Tantivy 排序后再次向 PostgreSQL 验证文章仍处于发布状态。
#[utoipa::path(get, path = "/api/v1/search", params(SearchQuery), responses((status = 200, body = ArticlePageResponse), (status = 400, body = ApiError)), tag = "articles")]
pub async fn search_articles(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    if query.q.chars().count() > 100
        || page == 0
        || per_page == 0
        || per_page > 50
        || page.saturating_mul(per_page) > 1000
    {
        return error(StatusCode::BAD_REQUEST, "invalid_search", "搜索参数无效");
    }
    if query.q.trim().is_empty() {
        return Json(ArticlePageResponse {
            items: Vec::new(),
            total: 0,
            page,
            per_page,
        })
        .into_response();
    }
    let engine = state.search.clone();
    let text = query.q;
    let result = tokio::task::spawn_blocking(move || engine.search(&text, page, per_page)).await;
    let (total, ids) = match result {
        Ok(Ok(value)) => value,
        _ => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "search_unavailable",
                "搜索暂时不可用",
            );
        }
    };
    if ids.is_empty() {
        return Json(ArticlePageResponse {
            items: Vec::new(),
            total: total as u64,
            page,
            per_page,
        })
        .into_response();
    }
    let rows = match article::Entity::find()
        .filter(article::Column::Id.is_in(ids.clone()))
        .filter(article::Column::Status.eq(ArticleStatus::Published))
        .all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "读取搜索结果失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut by_id: HashMap<i64, article::Model> =
        rows.into_iter().map(|row| (row.id, row)).collect();
    let items = ids
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .map(|row| ArticleSummaryResponse {
            public_id: row.public_id,
            slug: row.slug,
            title: row.title,
            summary: row.summary,
            published_at: row.published_at,
        })
        .collect();
    Json(ArticlePageResponse {
        items,
        total: total as u64,
        page,
        per_page,
    })
    .into_response()
}
