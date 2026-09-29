//! 管理员从 Notion 选择页面导入草稿，并按来源 ID 手动同步。

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    domain::article::render::render_document,
    domain::notion,
    entity::{
        article, article_revision, site_setting,
        status::{ArticleStatus, SearchAction},
        storage_provider,
    },
    infrastructure::{notion::NotionClient, notion_media, storage},
};

use super::super::{ApiError, AppState, error};
use super::{
    auth::require_admin,
    content::{self, AdminArticleResponse},
};

/// 管理端页面搜索参数，cursor 由 Notion 返回且只用于下一页。
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    pub q: Option<String>,
    pub cursor: Option<String>,
}

/// Notion 页面与本站文章的来源关联。
#[derive(Serialize, ToSchema)]
pub struct NotionPageItem {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub last_edited_at: DateTime<Utc>,
    pub article_public_id: Option<Uuid>,
    pub article_status: Option<&'static str>,
    pub synced_edited_at: Option<DateTime<Utc>>,
    pub local_changes: bool,
}

/// Notion 搜索结果，允许管理端按游标翻页。
#[derive(Serialize, ToSchema)]
pub struct NotionPageList {
    pub items: Vec<NotionPageItem>,
    pub next_cursor: Option<String>,
}

/// 本地修改过的文章需要显式确认覆盖。
#[derive(Default, Deserialize, ToSchema)]
pub struct SyncInput {
    #[serde(default)]
    pub force: bool,
}

/// 同步结果及转换时需要人工检查的内容。
#[derive(Serialize, ToSchema)]
pub struct SyncResponse {
    pub outcome: &'static str,
    pub article: AdminArticleResponse,
    pub warnings: Vec<String>,
}

fn notion_client(state: &AppState) -> Result<NotionClient, Box<Response>> {
    let Some(key) = state.notion_api_key.clone() else {
        return Err(Box::new(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "notion_not_configured",
            "未配置 NOTION_API_KEY",
        )));
    };
    NotionClient::new(key).map_err(|cause| {
        tracing::error!(error = %cause, "初始化 Notion 客户端失败");
        Box::new(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "服务暂时不可用",
        ))
    })
}

fn edited_locally(row: &article::Model) -> bool {
    row.notion_synced_at
        .is_some_and(|synced_at| row.updated_at > synced_at)
}

/// 搜索集成已共享页面；本站文章匹配使用 Notion 页面 UUID。
#[utoipa::path(get, path = "/api/v1/admin/notion/pages", params(PageQuery), responses((status = 200, body = NotionPageList), (status = 401, body = ApiError), (status = 503, body = ApiError)), tag = "admin")]
pub async fn pages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    if query
        .q
        .as_ref()
        .is_some_and(|value| value.chars().count() > 100)
        || query
            .cursor
            .as_ref()
            .is_some_and(|value| value.len() > 2048)
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_notion_query",
            "搜索参数过长",
        );
    }
    let client = match notion_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    let result = match client
        .search(query.q.as_deref().unwrap_or(""), query.cursor.as_deref())
        .await
    {
        Ok(result) => result,
        Err(cause) => {
            tracing::warn!(error = %cause, "搜索 Notion 页面失败");
            return error(
                StatusCode::BAD_GATEWAY,
                "notion_unavailable",
                "读取 Notion 失败，请检查页面共享权限",
            );
        }
    };
    let ids: Vec<Uuid> = result.pages.iter().map(|page| page.id).collect();
    let imported = if ids.is_empty() {
        Vec::new()
    } else {
        match article::Entity::find()
            .filter(article::Column::NotionPageId.is_in(ids))
            .all(&state.db)
            .await
        {
            Ok(rows) => rows,
            Err(cause) => {
                tracing::error!(error = %cause, "查询 Notion 来源文章失败");
                return error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "服务暂时不可用",
                );
            }
        }
    };
    let items = result
        .pages
        .into_iter()
        .map(|page| {
            let row = imported
                .iter()
                .find(|row| row.notion_page_id == Some(page.id));
            NotionPageItem {
                id: page.id,
                title: page.title,
                url: page.url,
                last_edited_at: page.last_edited_at,
                article_public_id: row.map(|row| row.public_id),
                article_status: row.map(|row| {
                    if row.status == ArticleStatus::Published {
                        "published"
                    } else {
                        "draft"
                    }
                }),
                synced_edited_at: row.and_then(|row| row.notion_last_edited_at),
                local_changes: row.is_some_and(edited_locally),
            }
        })
        .collect();
    Json(NotionPageList {
        items,
        next_cursor: result.next_cursor,
    })
    .into_response()
}

/// 首次创建草稿，后续同步保留文章 UUID、slug、发布状态与修订历史。
#[utoipa::path(post, path = "/api/v1/admin/notion/pages/{page_id}/sync", params(("page_id" = Uuid, Path)), request_body = SyncInput, responses((status = 200, body = SyncResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 409, body = ApiError)), tag = "admin")]
pub async fn sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(page_id): Path<Uuid>,
    Json(input): Json<SyncInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let client = match notion_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    let page = match client.page(page_id).await {
        Ok(page) => page,
        Err(cause) => {
            tracing::warn!(error = %cause, "读取 Notion 页面失败");
            return error(
                StatusCode::BAD_GATEWAY,
                "notion_unavailable",
                "页面不可读取，请确认已共享给集成",
            );
        }
    };
    let existing = match article::Entity::find()
        .filter(article::Column::NotionPageId.eq(page_id))
        .one(&state.db)
        .await
    {
        Ok(row) => row,
        Err(cause) => {
            tracing::error!(error = %cause, "查询 Notion 来源文章失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    if let Some(row) = &existing {
        if !input.force && row.notion_last_edited_at == Some(page.last_edited_at) {
            return Json(SyncResponse {
                outcome: "unchanged",
                article: row.clone().into(),
                warnings: Vec::new(),
            })
            .into_response();
        }
        if !input.force && edited_locally(row) {
            return error(
                StatusCode::CONFLICT,
                "notion_local_changes",
                "文章在本站已有修改，请确认覆盖后重试",
            );
        }
    }
    let blocks = match client.blocks(page_id).await {
        Ok(blocks) => blocks,
        Err(cause) => {
            tracing::warn!(error = %cause, "读取 Notion 块失败");
            return error(
                StatusCode::BAD_GATEWAY,
                "notion_unavailable",
                "读取 Notion 正文失败",
            );
        }
    };
    let provider = if notion_media::needs_storage(&blocks) {
        match current_provider(&state).await {
            Ok(Some(provider)) => Some(provider),
            Ok(None) => {
                return error(
                    StatusCode::CONFLICT,
                    "provider_missing",
                    "请先配置并启用图片存储提供商",
                );
            }
            Err(cause) => {
                tracing::error!(error = %cause, "读取 Notion 图片存储提供商失败");
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
    let (image_urls, imported_assets) = if let Some(provider) = &provider {
        if let Err(cause) = storage::configured(provider, &state.comment_hash_key) {
            tracing::warn!(error = %cause, "Notion 图片存储未配置");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "provider_unconfigured",
                "图片存储提供商配置不完整",
            );
        }
        match notion_media::import_images(&state.db, provider, &state.comment_hash_key, &blocks)
            .await
        {
            Ok(result) => result,
            Err(cause) => {
                tracing::warn!(error = %cause, "转存 Notion 图片失败");
                return error(
                    StatusCode::BAD_GATEWAY,
                    "notion_image_failed",
                    "Notion 图片转存失败",
                );
            }
        }
    } else {
        (Default::default(), Vec::new())
    };
    let converted = notion::convert(&page, &blocks, &image_urls);
    let document = json!({"type": "markdown", "source": converted.source});
    let html = match render_document(&document) {
        Ok(html) => html,
        Err(cause) => {
            if let Some(provider) = &provider {
                notion_media::cleanup(
                    &state.db,
                    provider,
                    &state.comment_hash_key,
                    &imported_assets,
                )
                .await;
            }
            tracing::warn!(error = %cause, "Notion 正文超过文章限制或格式无效");
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "notion_document_invalid",
                "Notion 正文过大或格式不受支持",
            );
        }
    };
    let title: String = page.title.trim().chars().take(160).collect();
    let now = Utc::now();
    let was_imported = existing.is_some();
    let result = async {
        let txn = state.db.begin().await?;
        let row = if let Some(old) = existing {
            article_revision::ActiveModel {
                article_id: Set(old.id),
                document: Set(old.document.clone()),
                saved_at: Set(now),
                ..Default::default()
            }
            .insert(&txn)
            .await?;
            let mut active = old.into_active_model();
            active.title = Set(title);
            active.summary = Set(converted.summary);
            active.document = Set(document);
            active.rendered_html = Set(html);
            active.notion_last_edited_at = Set(Some(page.last_edited_at));
            active.notion_synced_at = Set(Some(now));
            active.updated_at = Set(now);
            let row = active.update(&txn).await?;
            if row.status == ArticleStatus::Published {
                content::enqueue(&txn, row.id, SearchAction::Upsert, now).await?;
            }
            row
        } else {
            article::ActiveModel {
                id: NotSet,
                public_id: Set(Uuid::new_v4()),
                slug: Set(format!("notion-{}", page.id.simple())),
                title: Set(title),
                summary: Set(converted.summary),
                document: Set(document),
                rendered_html: Set(html),
                status: Set(ArticleStatus::Draft),
                cover_asset_id: Set(None),
                published_at: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                notion_page_id: Set(Some(page_id)),
                notion_last_edited_at: Set(Some(page.last_edited_at)),
                notion_synced_at: Set(Some(now)),
            }
            .insert(&txn)
            .await?
        };
        txn.commit().await?;
        Ok::<_, sea_orm::DbErr>(row)
    }
    .await;
    match result {
        Ok(row) => Json(SyncResponse {
            outcome: if was_imported { "updated" } else { "created" },
            article: row.into(),
            warnings: converted.warnings,
        })
        .into_response(),
        Err(cause) => {
            if let Some(provider) = &provider {
                notion_media::cleanup(
                    &state.db,
                    provider,
                    &state.comment_hash_key,
                    &imported_assets,
                )
                .await;
            }
            tracing::error!(error = %cause, "保存 Notion 同步文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "保存同步文章失败",
            )
        }
    }
}

/// 当前上传源与普通素材上传使用同一站点配置。
async fn current_provider(
    state: &AppState,
) -> Result<Option<storage_provider::Model>, sea_orm::DbErr> {
    let Some(settings) = site_setting::Entity::find_by_id(1).one(&state.db).await? else {
        return Ok(None);
    };
    let Some(id) = settings.active_provider_id else {
        return Ok(None);
    };
    Ok(storage_provider::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .filter(|row| row.upload_enabled))
}
