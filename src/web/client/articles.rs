//! 公开文章列表与详情接口。

use crate::domain::article;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entity::paid_column;

use super::reader;

use super::super::{
    ApiError, AppState, ArticleDetailResponse, ArticleHeadingResponse, ArticlePageResponse,
    ListArticlesQuery, error,
};

/// 列出未归属小册的已发布文章，拒绝零页码及超出上限的分页。
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
pub async fn get_article(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    match state.articles.get_published(&slug).await {
        Ok(Some(result)) => {
            let column = if let Some(id) = result.summary.paid_column_public_id {
                match paid_column::Entity::find()
                    .filter(paid_column::Column::PublicId.eq(id))
                    .one(&state.db)
                    .await
                {
                    Ok(Some(column)) => Some(column),
                    Ok(None) => {
                        return error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "invalid_column",
                            "文章专栏配置无效",
                        );
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "读取文章专栏失败");
                        return error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "internal_error",
                            "服务暂时不可用",
                        );
                    }
                }
            } else {
                None
            };
            let column_hidden = column.as_ref().is_some_and(|c| !c.visible);
            if column_hidden {
                let owns = match reader::get_session(&state, &headers).await {
                    Ok(Some(session)) => {
                        reader::owns_column(&state, session.reader_id, column.as_ref().unwrap().id)
                            .await
                            .unwrap_or(false)
                    }
                    _ => false,
                };
                if !owns {
                    return error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在");
                }
            }
            let locked = if result.summary.subscriber_only {
                let Some(column) = column.as_ref() else {
                    return error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "invalid_column",
                        "文章专栏配置无效",
                    );
                };
                let session = match reader::get_session(&state, &headers).await {
                    Ok(session) => session,
                    Err(err) => {
                        tracing::error!(error = %err, "读取读者会话失败");
                        return error(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "dependency_unavailable",
                            "服务暂时不可用",
                        );
                    }
                };
                match session {
                    Some(session) => {
                        match reader::owns_column(&state, session.reader_id, column.id).await {
                            Ok(owns) => !owns,
                            Err(err) => {
                                tracing::error!(error = %err, "校验专栏订阅失败");
                                return error(
                                    StatusCode::SERVICE_UNAVAILABLE,
                                    "dependency_unavailable",
                                    "服务暂时不可用",
                                );
                            }
                        }
                    }
                    None => true,
                }
            } else {
                false
            };
            let full_outline = article::render::heading_outline(&result.rendered_html);
            let rendered_html = if locked {
                article::render::preview_html(&result.rendered_html)
            } else {
                result.rendered_html
            };
            let visible_headings = if locked {
                article::render::heading_outline(&rendered_html).len()
            } else {
                full_outline.len()
            };
            let outline = full_outline
                .into_iter()
                .enumerate()
                .map(|(index, (level, label))| ArticleHeadingResponse {
                    level,
                    label,
                    available: !locked || index < visible_headings,
                })
                .collect();
            let mut response = Json(ArticleDetailResponse {
                public_id: result.summary.public_id,
                slug: result.summary.slug,
                title: result.summary.title,
                summary: result.summary.summary,
                published_at: result.summary.published_at,
                paid_column_public_id: result.summary.paid_column_public_id,
                subscriber_only: result.summary.subscriber_only,
                locked,
                column_slug: column.map(|column| column.slug),
                rendered_html,
                outline,
                cover_url: result.cover_url,
            })
            .into_response();
            if result.summary.subscriber_only || column_hidden {
                response
                    .headers_mut()
                    .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
                response
                    .headers_mut()
                    .insert(header::VARY, "Cookie".parse().unwrap());
            }
            response
        }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::to_bytes;
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, Database, EntityTrait, IntoActiveModel, Set};
    use uuid::Uuid;

    use crate::{
        agent::tools::ToolRegistry,
        entity::{article, column_subscription, paid_column, reader_user, status::ArticleStatus},
        infrastructure::{
            agent_skills::SkillStore, article_repository::SeaOrmArticleRepository,
            search::SearchEngine,
        },
        web::AppState,
    };

    use super::*;

    /// 专用已迁移数据库中验证：Cookie 本身不授权，订阅记录出现后才返回全文。
    #[tokio::test]
    #[ignore = "需要 PAID_TEST_DATABASE_URL 和 PAID_TEST_REDIS_URL 指向专用测试环境"]
    async fn subscription_controls_full_article_response() {
        let database_url = std::env::var("PAID_TEST_DATABASE_URL").expect("测试数据库地址");
        let redis_url = std::env::var("PAID_TEST_REDIS_URL").expect("测试 Redis 地址");
        let db = Database::connect(database_url).await.unwrap();
        let redis = redis::Client::open(redis_url).unwrap();
        let now = Utc::now();
        let suffix = Uuid::new_v4().simple().to_string();
        let column = paid_column::ActiveModel {
            visible: Set(true),
            id: sea_orm::NotSet,
            public_id: Set(Uuid::new_v4()),
            slug: Set(format!("paid-{suffix}")),
            title: Set("测试专栏".into()),
            description: Set(String::new()),
            price_cents: Set(100),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();
        let reader = reader_user::ActiveModel {
            id: sea_orm::NotSet,
            public_id: Set(Uuid::new_v4()),
            username: Set(format!("reader_{suffix}")),
            password_hash: Set("test-only".into()),
            created_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();
        let marker = format!("FULL_SECRET_{suffix}");
        let full_html = format!(
            "<h2>试看章节</h2><p>{}</p><h2>订阅章节</h2><p>{marker}</p>",
            "试看正文".repeat(50)
        );
        let row = article::ActiveModel {
            id: sea_orm::NotSet,
            public_id: Set(Uuid::new_v4()),
            slug: Set(format!("chapter-{suffix}")),
            title: Set("测试篇章".into()),
            summary: Set(None),
            document: Set(serde_json::json!({"type":"markdown","source":marker})),
            rendered_html: Set(full_html),
            status: Set(ArticleStatus::Published),
            cover_asset_id: Set(None),
            published_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
            notion_page_id: Set(None),
            notion_last_edited_at: Set(None),
            notion_synced_at: Set(None),
            paid_column_public_id: Set(Some(column.public_id)),
            subscriber_only: Set(true),
            preview_document: Set(Some(
                serde_json::json!({"type":"markdown","source":"公开试看"}),
            )),
            // 故意保留包含全文的旧试看，验证公开接口只采用最新的固定比例裁剪。
            preview_html: Set(Some(format!("<p>{marker}</p>"))),
        }
        .insert(&db)
        .await
        .unwrap();
        let state = AppState {
            tools: Arc::new(
                ToolRegistry::new(std::env::temp_dir().join("oxide-pay-test-tools.json"), None)
                    .unwrap(),
            ),
            skills: Arc::new(SkillStore::new(
                std::env::temp_dir().join("oxide-pay-test-skills"),
            )),
            articles: Arc::new(SeaOrmArticleRepository::new(db.clone())),
            db: db.clone(),
            redis: redis.clone(),
            comment_hash_key: Arc::from("test-secret-with-at-least-32-bytes"),
            session_secure: false,
            public_base_url: Arc::from("http://127.0.0.1:3000"),
            search: Arc::new(SearchEngine::in_memory()),
            notion_api_key: None,
            wechat_pay: None,
        };
        // 普通写作的分页和总数必须同时排除小册免费章节与付费章节。
        let mut free_chapter = row.clone().into_active_model();
        free_chapter.id = sea_orm::NotSet;
        free_chapter.public_id = Set(Uuid::new_v4());
        free_chapter.slug = Set(format!("free-{suffix}"));
        free_chapter.subscriber_only = Set(false);
        let free_chapter = free_chapter.insert(&db).await.unwrap();
        let mut writing = free_chapter.clone().into_active_model();
        writing.id = sea_orm::NotSet;
        writing.public_id = Set(Uuid::new_v4());
        writing.slug = Set(format!("writing-{suffix}"));
        writing.paid_column_public_id = Set(None);
        let writing = writing.insert(&db).await.unwrap();
        let page = state.articles.list_published(1, 1).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].slug, writing.slug);
        let next = state.articles.list_published(2, 1).await.unwrap();
        assert_eq!(next.total, 1);
        assert!(next.items.is_empty());
        assert!(
            state
                .articles
                .get_published(&free_chapter.slug)
                .await
                .unwrap()
                .is_some()
        );
        let token = Uuid::new_v4().to_string();
        let mut connection = redis.get_multiplexed_async_connection().await.unwrap();
        let session = reader::ReaderSession {
            reader_id: reader.id,
            username: reader.username.clone(),
            csrf_token: "test".into(),
        };
        redis::cmd("SET")
            .arg(format!("reader-session:{token}"))
            .arg(serde_json::to_string(&session).unwrap())
            .arg("EX")
            .arg(60)
            .query_async::<()>(&mut connection)
            .await
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("oxide_reader={token}").parse().unwrap(),
        );
        let read = |state: AppState, slug: String, headers: HeaderMap| async move {
            let response = get_article(State(state), Path(slug), headers).await;
            let body = to_bytes(response.into_body(), 1_000_000).await.unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()
        };
        let anonymous = read(state.clone(), row.slug.clone(), HeaderMap::new()).await;
        assert_eq!(anonymous["locked"], true);
        assert!(!anonymous.to_string().contains(&marker));
        assert_eq!(anonymous["outline"][0]["label"], "试看章节");
        assert_eq!(anonymous["outline"][0]["available"], true);
        assert_eq!(anonymous["outline"][1]["label"], "订阅章节");
        assert_eq!(anonymous["outline"][1]["available"], false);
        assert_eq!(
            anonymous["rendered_html"],
            crate::domain::article::render::preview_html(&row.rendered_html)
        );
        let unpaid = read(state.clone(), row.slug.clone(), headers.clone()).await;
        assert_eq!(unpaid["locked"], true);
        assert!(!unpaid.to_string().contains(&marker));
        assert_eq!(unpaid["rendered_html"], anonymous["rendered_html"]);
        let subscription = column_subscription::ActiveModel {
            id: sea_orm::NotSet,
            reader_id: Set(reader.id),
            paid_column_id: Set(column.id),
            created_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();
        let paid = read(state, row.slug.clone(), headers).await;
        assert_eq!(paid["locked"], false);
        assert_eq!(paid["outline"][1]["available"], true);
        assert!(paid["rendered_html"].as_str().unwrap().contains(&marker));
        redis::cmd("DEL")
            .arg(format!("reader-session:{token}"))
            .query_async::<()>(&mut connection)
            .await
            .unwrap();
        column_subscription::Entity::delete_by_id(subscription.id)
            .exec(&db)
            .await
            .unwrap();
        for id in [row.id, free_chapter.id, writing.id] {
            article::Entity::delete_by_id(id).exec(&db).await.unwrap();
        }
        reader_user::Entity::delete_by_id(reader.id)
            .exec(&db)
            .await
            .unwrap();
        paid_column::Entity::delete_by_id(column.id)
            .exec(&db)
            .await
            .unwrap();
    }
}
