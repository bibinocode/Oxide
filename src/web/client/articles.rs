//! 公开文章列表与详情接口。

use crate::domain::article;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::{
    ApiError, AppState, ArticleDetailResponse, ArticlePageResponse, ListArticlesQuery, error,
};

/// 列出已发布文章，拒绝零页码及超出上限的分页。
#[utoipa::path(get, path = "/api/v1/articles", params(ListArticlesQuery), responses(
    (status = 200, body = ArticlePageResponse),
    (status = 400, body = ApiError),
    (status = 500, body = ApiError)
), tag = "articles")]
pub async fn list_articles(
    State(state): State<AppState>,
    query: Result<Query<ListArticlesQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_pagination",
                "分页参数无效",
            );
        }
    };
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    if page == 0
        || per_page == 0
        || per_page > article::MAX_PAGE_SIZE
        || page - 1 > (i64::MAX as u64) / per_page
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_pagination",
            "分页参数无效",
        );
    }
    match state.articles.list_published(page, per_page).await {
        Ok(result) => Json(ArticlePageResponse {
            items: result.items.into_iter().map(Into::into).collect(),
            page,
            per_page,
            total: result.total,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "查询文章列表失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 按 slug 获取公开文章；草稿与不存在使用同一 404 响应。
#[utoipa::path(get, path = "/api/v1/articles/{slug}", params(("slug" = String, Path, description = "文章永久链接标识")), responses(
    (status = 200, body = ArticleDetailResponse),
    (status = 404, body = ApiError),
    (status = 500, body = ApiError)
), tag = "articles")]
pub async fn get_article(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    match state.articles.get_published(&slug).await {
        Ok(Some(result)) => Json(ArticleDetailResponse {
            public_id: result.summary.public_id,
            slug: result.summary.slug,
            title: result.summary.title,
            summary: result.summary.summary,
            published_at: result.summary.published_at,
            rendered_html: result.rendered_html,
            cover_url: result.cover_url,
        })
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "查询文章详情失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}
