//! 站点信息、分类标签和 RSS 订阅接口。

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use sea_orm::{
    ColumnTrait, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
    sea_query::{Expr, ExprTrait, Query as SeaQuery},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::entity::{
    article, article_category, article_tag, category, site_setting, status::ArticleStatus, tag,
};

use super::super::{
    ApiError, AppState, ArticlePageResponse, ArticleSummaryResponse, ListArticlesQuery, error,
};

/// 分类/标签列表的轻量字段投影。
#[derive(FromQueryResult)]
struct ArticleRow {
    public_id: uuid::Uuid,
    slug: String,
    title: String,
    summary: Option<String>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    paid_column_public_id: Option<uuid::Uuid>,
    subscriber_only: bool,
}

/// 公开站点配置；不输出当前存储提供商等内部配置。
#[derive(Serialize, ToSchema)]
pub struct SiteResponse {
    /// 站点名称。
    pub site_name: String,
    /// 站点简介。
    pub description: Option<String>,
    /// 站点绝对地址。
    pub base_url: String,
    /// 可开关的公开模块与页脚内容。
    pub presentation: crate::domain::site::SitePresentation,
}

/// 分类或标签的公开模型。
#[derive(Serialize, ToSchema)]
pub struct TaxonomyResponse {
    /// 是否公开展示，管理端仍可恢复隐藏条目。
    pub visible: bool,
    /// 展示名称。
    pub name: String,
    /// URL 标识。
    pub slug: String,
}

/// 读取站点设置；新建数据库在管理端保存前使用安全的默认展示值。
#[utoipa::path(get, path = "/api/v1/site", responses((status = 200, body = SiteResponse), (status = 500, body = ApiError)), tag = "site")]
pub async fn site(State(state): State<AppState>) -> Response {
    match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(Some(row)) => Json(SiteResponse {
            site_name: row.site_name,
            description: row.description,
            base_url: row.base_url,
            presentation: row
                .presentation
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default(),
        })
        .into_response(),
        Ok(None) => Json(SiteResponse {
            site_name: "Oxide".into(),
            description: None,
            base_url: state.public_base_url.to_string(),
            presentation: Default::default(),
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取站点信息失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 列出全部分类，管理端可通过单独接口维护。
#[utoipa::path(get, path = "/api/v1/categories", responses((status = 200, body = Vec<TaxonomyResponse>), (status = 500, body = ApiError)), tag = "taxonomy")]
pub async fn categories(State(state): State<AppState>) -> Response {
    match category::Entity::find()
        .filter(category::Column::Visible.eq(true))
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
        Err(err) => {
            tracing::error!(error = %err, "读取分类失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 列出全部标签。
#[utoipa::path(get, path = "/api/v1/tags", responses((status = 200, body = Vec<TaxonomyResponse>), (status = 500, body = ApiError)), tag = "taxonomy")]
pub async fn tags(State(state): State<AppState>) -> Response {
    match tag::Entity::find()
        .filter(tag::Column::Visible.eq(true))
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
        Err(err) => {
            tracing::error!(error = %err, "读取标签失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 分类文章仅包含已发布内容。
#[utoipa::path(get, path = "/api/v1/categories/{slug}/articles", params(("slug" = String, Path), ListArticlesQuery), responses((status = 200, body = ArticlePageResponse), (status = 404, body = ApiError)), tag = "taxonomy")]
pub async fn category_articles(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<ListArticlesQuery>,
) -> Response {
    let category = match category::Entity::find()
        .filter(category::Column::Slug.eq(slug))
        .filter(category::Column::Visible.eq(true))
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
    let subquery = SeaQuery::select()
        .column(article_category::Column::ArticleId)
        .from(article_category::Entity)
        .and_where(Expr::col(article_category::Column::CategoryId).eq(category.id))
        .to_owned();
    filtered_articles(&state, article::Column::Id.in_subquery(subquery), query).await
}

/// 标签文章仅包含已发布内容。
#[utoipa::path(get, path = "/api/v1/tags/{slug}/articles", params(("slug" = String, Path), ListArticlesQuery), responses((status = 200, body = ArticlePageResponse), (status = 404, body = ApiError)), tag = "taxonomy")]
pub async fn tag_articles(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<ListArticlesQuery>,
) -> Response {
    let tag = match tag::Entity::find()
        .filter(tag::Column::Slug.eq(slug))
        .filter(tag::Column::Visible.eq(true))
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
    let subquery = SeaQuery::select()
        .column(article_tag::Column::ArticleId)
        .from(article_tag::Entity)
        .and_where(Expr::col(article_tag::Column::TagId).eq(tag.id))
        .to_owned();
    filtered_articles(&state, article::Column::Id.in_subquery(subquery), query).await
}

/// 分类和标签共用的分页投影，只向前端返回公开字段。
async fn filtered_articles(
    state: &AppState,
    condition: sea_orm::sea_query::SimpleExpr,
    query: ListArticlesQuery,
) -> Response {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    if page == 0 || per_page == 0 || per_page > 50 || page > 100_000 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_pagination",
            "分页参数无效",
        );
    }
    let result = async {
        let base = article::Entity::find()
            .filter(article::Column::Status.eq(ArticleStatus::Published))
            .filter(condition);
        let total = base.clone().count(&state.db).await?;
        let rows = base
            .select_only()
            .columns([
                article::Column::PublicId,
                article::Column::Slug,
                article::Column::Title,
                article::Column::Summary,
                article::Column::PublishedAt,
                article::Column::PaidColumnPublicId,
                article::Column::SubscriberOnly,
            ])
            .order_by_desc(article::Column::PublishedAt)
            .limit(per_page)
            .offset((page - 1) * per_page)
            .into_model::<ArticleRow>()
            .all(&state.db)
            .await?;
        Ok::<_, sea_orm::DbErr>((total, rows))
    }
    .await;
    match result {
        Ok((total, rows)) => Json(ArticlePageResponse {
            page,
            per_page,
            total,
            items: rows
                .into_iter()
                .map(|row| ArticleSummaryResponse {
                    public_id: row.public_id,
                    slug: row.slug,
                    title: row.title,
                    summary: row.summary,
                    cover_url: None,
                    published_at: row.published_at,
                    paid_column_public_id: row.paid_column_public_id,
                    subscriber_only: row.subscriber_only,
                })
                .collect(),
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取分类文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// RSS 2.0 仅包含已发布文章，链接使用站点绝对地址。
#[utoipa::path(get, path = "/feed.xml", responses((status = 200, content_type = "application/rss+xml"), (status = 500, body = ApiError)), tag = "site")]
pub async fn feed(State(state): State<AppState>) -> Response {
    let settings = match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(row) => row,
        Err(err) => {
            tracing::error!(error = %err, "读取 RSS 设置失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let articles = match state.articles.list_published(1, 30).await {
        Ok(page) => page.items,
        Err(err) => {
            tracing::error!(error = %err, "读取 RSS 文章失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let base_url = settings
        .as_ref()
        .map_or(state.public_base_url.as_ref(), |row| row.base_url.as_str())
        .trim_end_matches('/');
    let items: Vec<rss::Item> = articles
        .into_iter()
        .map(|article| {
            let link = format!("{base_url}/articles/{}", article.slug);
            rss::ItemBuilder::default()
                .title(Some(article.title))
                .link(Some(link.clone()))
                .description(article.summary)
                .pub_date(article.published_at.map(|date| date.to_rfc2822()))
                .guid(Some(
                    rss::GuidBuilder::default()
                        .value(link)
                        .permalink(true)
                        .build(),
                ))
                .build()
        })
        .collect();
    let channel = rss::ChannelBuilder::default()
        .title(
            settings
                .as_ref()
                .map_or("Oxide", |row| row.site_name.as_str())
                .to_owned(),
        )
        .link(base_url.to_owned())
        .description(
            settings
                .as_ref()
                .and_then(|row| row.description.clone())
                .unwrap_or_default(),
        )
        .items(items)
        .build();
    (
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        channel.to_string(),
    )
        .into_response()
}
