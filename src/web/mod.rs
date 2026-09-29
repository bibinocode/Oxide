//! Axum 路由、响应模型和 OpenAPI 文档。

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::domain::article::{self, ArticleRepository};
use crate::infrastructure::search::SearchEngine;

pub mod admin;
pub mod client;

/// 各路由共享的外部资源及可替换文章仓储。
#[derive(Clone)]
pub struct AppState {
    /// 公开文章查询接口。
    pub articles: Arc<dyn ArticleRepository>,
    /// PostgreSQL 连接池，用于就绪探针。
    pub db: DatabaseConnection,
    /// Redis 客户端，用于就绪探针。
    pub redis: redis::Client,
    /// 评论邮箱摘要密钥，只在写入处理器使用。
    pub comment_hash_key: Arc<str>,
    /// 会话 Cookie 是否带 Secure 标记。
    pub session_secure: bool,
    /// RSS 与公开链接的回退基础地址。
    pub public_base_url: Arc<str>,
    /// 持久化搜索索引及分词器。
    pub search: Arc<SearchEngine>,
    /// Notion 集成密钥；未配置时管理端同步接口返回明确错误。
    pub notion_api_key: Option<Arc<str>>,
}

/// 统一错误格式，便于前端按 code 分支处理。
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    /// 稳定的机器可读错误码。
    pub code: &'static str,
    /// 面向用户的简短说明。
    pub message: &'static str,
}

/// 文章列表的分页参数。
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListArticlesQuery {
    /// 从 1 开始的页码，默认 1。
    pub page: Option<u64>,
    /// 每页条数，默认 20，上限 50。
    pub per_page: Option<u64>,
}

/// 公开文章列表项，不含内部主键和未公开正文。
#[derive(Debug, Serialize, ToSchema)]
pub struct ArticleSummaryResponse {
    /// 对外 UUID。
    pub public_id: Uuid,
    /// 永久链接标识。
    pub slug: String,
    /// 标题。
    pub title: String,
    /// 可选摘要。
    pub summary: Option<String>,
    /// 首次发布时间。
    pub published_at: Option<DateTime<Utc>>,
}

impl From<article::ArticleSummary> for ArticleSummaryResponse {
    fn from(value: article::ArticleSummary) -> Self {
        Self {
            public_id: value.public_id,
            slug: value.slug,
            title: value.title,
            summary: value.summary,
            published_at: value.published_at,
        }
    }
}

/// 分页文章响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct ArticlePageResponse {
    /// 当前页数据。
    pub items: Vec<ArticleSummaryResponse>,
    /// 当前页码。
    pub page: u64,
    /// 每页上限。
    pub per_page: u64,
    /// 符合条件的文章总数。
    pub total: u64,
}

/// 文章详情响应，HTML 已在发布流程中生成并净化。
#[derive(Debug, Serialize, ToSchema)]
pub struct ArticleDetailResponse {
    /// 对外 UUID。
    pub public_id: Uuid,
    /// 永久链接标识。
    pub slug: String,
    /// 标题。
    pub title: String,
    /// 可选摘要。
    pub summary: Option<String>,
    /// 首次发布时间。
    pub published_at: Option<DateTime<Utc>>,
    /// 已保存的公开 HTML。
    pub rendered_html: String,
    /// 公开封面素材路径。
    pub cover_url: Option<String>,
}

/// 健康探针响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// 探针状态。
    pub status: &'static str,
}

/// OpenAPI 入口；前端可从 `/openapi.json` 生成 TypeScript 类型。
#[derive(OpenApi)]
#[openapi(
    paths(client::articles::list_articles, client::articles::get_article, live, ready, admin::auth::login, admin::auth::current, admin::auth::logout,
        admin::preview::preview, admin::cover::get, admin::cover::update,
        admin::content::list, admin::content::create, admin::content::get, admin::content::update,
        admin::content::publish, admin::content::unpublish, admin::content::revisions, admin::content::delete,
        client::content::site, client::content::categories, client::content::tags, client::content::feed,
        client::content::category_articles, client::content::tag_articles, client::search::search_articles,
        client::comments::list, client::comments::create, client::comments::avatar, admin::comments::admin_list, admin::comments::review, admin::comments::delete,
        admin::settings::update_site, admin::settings::create_category, admin::settings::update_category,
        admin::settings::delete_category, admin::settings::create_tag, admin::settings::update_tag,
        admin::settings::delete_tag, admin::taxonomy::get, admin::taxonomy::update,
        admin::storage::providers, admin::storage::create_provider, admin::storage::update_provider, admin::storage::activate_provider,
        admin::storage::delete_provider, admin::storage::assets, admin::storage::upload, admin::storage::set_visibility,
        admin::storage::delete_asset, client::media::media,
        admin::agent::providers, admin::agent::create_provider, admin::agent::update_provider,
        admin::agent::delete_provider, admin::agent::bindings, admin::agent::bind_task,
        admin::agent::summary::generate, admin::agent::writing::generate,
        admin::agent::writing::stream,
        client::link_preview::preview, admin::notion::pages, admin::notion::sync),
    components(schemas(ApiError, ArticleSummaryResponse, ArticlePageResponse, ArticleDetailResponse, HealthResponse,
        admin::auth::LoginRequest, admin::auth::SessionResponse, admin::auth::OkResponse,
        admin::preview::PreviewInput, admin::preview::PreviewResponse, admin::cover::CoverInput, admin::cover::CoverResponse,
        admin::content::ArticleInput, admin::content::AdminArticleResponse, admin::content::AdminArticlePage,
        admin::content::RevisionResponse, client::content::SiteResponse, client::content::TaxonomyResponse,
        client::comments::CommentInput, client::comments::CommentCreated, client::comments::CommentResponse,
        admin::comments::AdminCommentResponse, admin::comments::ReviewInput, admin::comments::ReviewStatus,
        admin::settings::SiteInput, admin::settings::TaxonomyInput, admin::taxonomy::ArticleTaxonomy,
        admin::storage::ProviderInput, admin::storage::ProviderResponse, admin::storage::AssetResponse, admin::storage::VisibilityInput,
        admin::agent::ProviderInput, admin::agent::ProviderResponse, admin::agent::BindingInput,
        admin::agent::BindingResponse, admin::agent::summary::SummaryInput,
        admin::agent::summary::SummaryResponse, admin::agent::writing::WritingInput,
        admin::agent::writing::WritingResponse, crate::agent::writing::WritingAction,
        client::link_preview::LinkPreview,
        admin::notion::NotionPageItem, admin::notion::NotionPageList,
        admin::notion::SyncInput, admin::notion::SyncResponse)),
    tags((name = "articles", description = "公开文章"), (name = "health", description = "进程健康检查"),
        (name = "admin", description = "管理员内容和会话"), (name = "site", description = "站点和订阅"),
        (name = "taxonomy", description = "分类和标签"), (name = "comments", description = "匿名评论"),
        (name = "agent", description = "Agent 核心模型配置"))
)]
pub struct ApiDoc;

/// 组装 API 和文档路由。
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/admin/preview", post(admin::preview::preview))
        .route(
            "/api/v1/admin/articles/{public_id}/cover",
            get(admin::cover::get).put(admin::cover::update),
        )
        .route("/api/v1/articles", get(client::articles::list_articles))
        .route(
            "/api/v1/articles/{slug}",
            get(client::articles::get_article),
        )
        .route("/api/v1/link-preview", get(client::link_preview::preview))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/api/v1/site", get(client::content::site))
        .route("/api/v1/categories", get(client::content::categories))
        .route("/api/v1/tags", get(client::content::tags))
        .route(
            "/api/v1/categories/{slug}/articles",
            get(client::content::category_articles),
        )
        .route(
            "/api/v1/tags/{slug}/articles",
            get(client::content::tag_articles),
        )
        .route("/api/v1/search", get(client::search::search_articles))
        .route("/feed.xml", get(client::content::feed))
        .route(
            "/api/v1/articles/{slug}/comments",
            get(client::comments::list).post(client::comments::create),
        )
        .route(
            "/api/v1/comments/{public_id}/avatar.svg",
            get(client::comments::avatar),
        )
        .route("/api/v1/admin/comments", get(admin::comments::admin_list))
        .route(
            "/api/v1/admin/comments/{public_id}",
            axum::routing::patch(admin::comments::review).delete(admin::comments::delete),
        )
        .route(
            "/api/v1/admin/site",
            axum::routing::put(admin::settings::update_site),
        )
        .route(
            "/api/v1/admin/categories",
            post(admin::settings::create_category),
        )
        .route(
            "/api/v1/admin/categories/{slug}",
            axum::routing::put(admin::settings::update_category)
                .delete(admin::settings::delete_category),
        )
        .route("/api/v1/admin/tags", post(admin::settings::create_tag))
        .route(
            "/api/v1/admin/tags/{slug}",
            axum::routing::put(admin::settings::update_tag).delete(admin::settings::delete_tag),
        )
        .route("/api/v1/admin/login", post(admin::auth::login))
        .route("/api/v1/admin/logout", post(admin::auth::logout))
        .route("/api/v1/admin/session", get(admin::auth::current))
        .route("/api/v1/admin/notion/pages", get(admin::notion::pages))
        .route(
            "/api/v1/admin/notion/pages/{page_id}/sync",
            post(admin::notion::sync),
        )
        .route(
            "/api/v1/admin/articles",
            get(admin::content::list).post(admin::content::create),
        )
        .route(
            "/api/v1/admin/articles/{public_id}",
            get(admin::content::get)
                .put(admin::content::update)
                .delete(admin::content::delete),
        )
        .route(
            "/api/v1/admin/articles/{public_id}/publish",
            post(admin::content::publish),
        )
        .route(
            "/api/v1/admin/articles/{public_id}/unpublish",
            post(admin::content::unpublish),
        )
        .route(
            "/api/v1/admin/articles/{public_id}/revisions",
            get(admin::content::revisions),
        )
        .route(
            "/api/v1/admin/articles/{public_id}/taxonomy",
            get(admin::taxonomy::get).put(admin::taxonomy::update),
        )
        .route(
            "/api/v1/admin/storage-providers",
            get(admin::storage::providers).post(admin::storage::create_provider),
        )
        .route(
            "/api/v1/admin/agent/providers",
            get(admin::agent::providers).post(admin::agent::create_provider),
        )
        .route(
            "/api/v1/admin/agent/providers/{id}",
            axum::routing::put(admin::agent::update_provider).delete(admin::agent::delete_provider),
        )
        .route("/api/v1/admin/agent/bindings", get(admin::agent::bindings))
        .route(
            "/api/v1/admin/agent/summary",
            post(admin::agent::summary::generate),
        )
        .route(
            "/api/v1/admin/agent/writing",
            post(admin::agent::writing::generate),
        )
        .route(
            "/api/v1/admin/agent/writing/stream",
            post(admin::agent::writing::stream),
        )
        .route(
            "/api/v1/admin/agent/bindings/{task}",
            axum::routing::put(admin::agent::bind_task),
        )
        .route(
            "/api/v1/admin/storage-providers/{id}",
            axum::routing::put(admin::storage::update_provider)
                .delete(admin::storage::delete_provider),
        )
        .route(
            "/api/v1/admin/storage-providers/{id}/activate",
            post(admin::storage::activate_provider),
        )
        .route(
            "/api/v1/admin/assets",
            get(admin::storage::assets)
                .post(admin::storage::upload)
                .layer(axum::extract::DefaultBodyLimit::max(9 * 1024 * 1024)),
        )
        .route(
            "/api/v1/admin/assets/{public_id}",
            axum::routing::patch(admin::storage::set_visibility)
                .delete(admin::storage::delete_asset),
        )
        .route("/media/{public_id}", get(client::media::media))
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// 仅证明 HTTP 进程仍可响应，不检查外部依赖。
#[utoipa::path(get, path = "/health/live", responses((status = 200, body = HealthResponse)), tag = "health")]
pub async fn live() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// 同时检查 PostgreSQL 与 Redis，供部署系统判定是否可接流量。
#[utoipa::path(get, path = "/health/ready", responses((status = 200, body = HealthResponse), (status = 503, body = ApiError)), tag = "health")]
pub async fn ready(State(state): State<AppState>) -> Response {
    let database = state.db.ping().await;
    let redis = async {
        let mut connection = state.redis.get_multiplexed_async_connection().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
    }
    .await;
    if let (Ok(()), Ok(_)) = (database, redis) {
        Json(HealthResponse { status: "ok" }).into_response()
    } else {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "依赖服务不可用",
        )
    }
}

/// 将公开错误码与 HTTP 状态合并为统一响应。
fn error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (status, Json(ApiError { code, message })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    /// 固定返回公开文章，用于验证 HTTP 契约而非数据库实现。
    struct FakeArticles;

    #[async_trait]
    impl ArticleRepository for FakeArticles {
        async fn list_published(&self, _page: u64, _per_page: u64) -> Result<article::ArticlePage> {
            Ok(article::ArticlePage {
                items: vec![sample()],
                total: 1,
            })
        }

        async fn get_published(&self, slug: &str) -> Result<Option<article::ArticleDetail>> {
            Ok((slug == "hello").then(|| article::ArticleDetail {
                summary: sample(),
                rendered_html: "<p>你好</p>".into(),
                cover_url: None,
            }))
        }
    }

    /// 测试专用文章，没有数据库内部 ID。
    fn sample() -> article::ArticleSummary {
        article::ArticleSummary {
            public_id: Uuid::nil(),
            slug: "hello".into(),
            title: "Hello".into(),
            summary: Some("摘要".into()),
            published_at: None,
        }
    }

    /// 使用无连接的探针依赖，公开文章路由由假仓储提供。
    fn app() -> Router {
        router(AppState {
            articles: Arc::new(FakeArticles),
            db: DatabaseConnection::default(),
            redis: redis::Client::open("redis://127.0.0.1/").unwrap(),
            comment_hash_key: Arc::from("test-secret-with-at-least-32-bytes"),
            session_secure: false,
            public_base_url: Arc::from("http://127.0.0.1:3000"),
            search: Arc::new(SearchEngine::in_memory()),
            notion_api_key: None,
        })
    }

    /// 发起完整 HTTP 请求并读取 JSON 响应。
    async fn get(path: &str) -> (StatusCode, serde_json::Value) {
        let response = app()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn public_articles_hide_internal_ids() {
        let (status, list) = get("/api/v1/articles?page=1&per_page=10").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["total"], 1);
        assert_eq!(list["items"][0]["public_id"], Uuid::nil().to_string());
        assert!(list["items"][0].get("id").is_none());
        let (status, detail) = get("/api/v1/articles/hello").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["rendered_html"], "<p>你好</p>");
        assert!(detail.get("id").is_none());
    }

    #[tokio::test]
    async fn pagination_and_missing_article_have_stable_errors() {
        let (status, body) = get("/api/v1/articles?page=0").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid_pagination");
        let (status, body) = get("/api/v1/articles?page=abc").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid_pagination");
        let (status, body) = get("/api/v1/articles/draft").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "article_not_found");
    }

    #[tokio::test]
    async fn openapi_lists_public_routes() {
        let (status, doc) = get("/openapi.json").await;
        assert_eq!(status, StatusCode::OK);
        assert!(doc["paths"]["/api/v1/articles"].is_object());
        assert!(doc["paths"]["/api/v1/articles/{slug}"].is_object());
        assert!(doc["paths"]["/api/v1/admin/notion/pages"].is_object());
        assert!(doc["paths"]["/api/v1/admin/notion/pages/{page_id}/sync"].is_object());
        assert!(doc["paths"]["/api/v1/admin/agent/summary"].is_object());
        assert!(doc["paths"]["/api/v1/admin/agent/writing"].is_object());
        assert!(doc["paths"]["/api/v1/admin/agent/writing/stream"].is_object());
        let (status, body) = get("/health/live").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
    }
}
