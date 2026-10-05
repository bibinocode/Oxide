//! 管理员文章草稿、发布和修订 API。

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    domain::article::render::render_document,
    entity::{
        article, article_category, article_revision, article_tag, comment, paid_column, search_job,
        status::{ArticleStatus, SearchAction, SearchJobStatus},
    },
};

use super::super::{ApiError, AppState, error};
use super::auth::require_admin;

/// 管理文章列表的分页参数。
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AdminPageQuery {
    /// 从 1 开始的页码。
    pub page: Option<u64>,
    /// 指定小册时仅返回该小册的篇章，包括未发布草稿。
    pub column_public_id: Option<Uuid>,
}

/// 文章写入内容；发布状态由单独接口控制。
#[derive(Deserialize, ToSchema)]
pub struct ArticleInput {
    /// 自动保存使用最后已知版本，防止多标签页或外部同步静默覆盖。
    #[serde(default)]
    pub expected_updated_at: Option<DateTime<Utc>>,
    /// 自动保存仅允许写草稿；已发布正文必须由用户显式保存。
    #[serde(default)]
    pub draft_only: bool,
    /// 唯一 slug，仅允许小写字母、数字和中划线。
    pub slug: String,
    /// 文章标题。
    pub title: String,
    /// 可选摘要。
    pub summary: Option<String>,
    /// Markdown 文档（type=markdown, source=原文）；兼容已有 Tiptap JSON 文档。
    pub document: Value,
    /// 省略时保留已有访问配置，避免旧客户端意外解除付费限制。
    #[serde(default)]
    pub access: Option<ArticleAccessInput>,
}

/// 作者只指定所属专栏和付费开关；试看比例由服务端固定。
#[derive(Clone, Deserialize, Serialize, ToSchema)]
pub struct ArticleAccessInput {
    /// 不属于专栏时为空；付费篇章必须指定专栏。
    pub column_public_id: Option<Uuid>,
    /// 免费全文篇章为 false，试看 30% 的付费篇章为 true。
    pub subscriber_only: bool,
}

/// 管理端文章响应；内部主键不对外输出。
#[derive(Serialize, ToSchema)]
pub struct AdminArticleResponse {
    /// 公开 UUID。
    pub public_id: Uuid,
    /// 永久链接标识。
    pub slug: String,
    /// 标题。
    pub title: String,
    /// 摘要。
    pub summary: Option<String>,
    /// 草稿或已发布。
    pub status: &'static str,
    /// 编辑器文档。
    pub document: Value,
    /// 首次发布时间。
    pub published_at: Option<DateTime<Utc>>,
    /// 最后更新时间。
    pub updated_at: DateTime<Utc>,
    /// Notion 来源页；本站创建的文章为空。
    pub notion_page_id: Option<Uuid>,
    /// Notion 页面上次同步的编辑时间。
    pub notion_last_edited_at: Option<DateTime<Utc>>,
    /// 作者可编辑的访问配置。
    pub access: ArticleAccessInput,
}

impl From<article::Model> for AdminArticleResponse {
    fn from(row: article::Model) -> Self {
        Self {
            public_id: row.public_id,
            slug: row.slug,
            title: row.title,
            summary: row.summary,
            status: if row.status == ArticleStatus::Published {
                "published"
            } else {
                "draft"
            },
            document: row.document,
            published_at: row.published_at,
            updated_at: row.updated_at,
            notion_page_id: row.notion_page_id,
            notion_last_edited_at: row.notion_last_edited_at,
            access: ArticleAccessInput {
                column_public_id: row.paid_column_public_id,
                subscriber_only: row.subscriber_only,
            },
        }
    }
}

/// 管理端文章列表。
#[derive(Serialize, ToSchema)]
pub struct AdminArticlePage {
    /// 当前页文章。
    pub items: Vec<AdminArticleResponse>,
    /// 当前页码。
    pub page: u64,
    /// 总文章数。
    pub total: u64,
}

/// 修订记录响应；修订 ID 为内部主键，只返回保存时间和文档。
#[derive(Serialize, ToSchema)]
pub struct RevisionResponse {
    /// 保存时间。
    pub saved_at: DateTime<Utc>,
    /// 当时的编辑器文档。
    pub document: Value,
}

/// 校验文章输入并生成公开 HTML。
fn validate(input: &ArticleInput) -> Result<String, &'static str> {
    let slug = input.slug.as_bytes();
    if slug.len() < 3
        || slug.len() > 100
        || slug.first() == Some(&b'-')
        || slug.last() == Some(&b'-')
        || !slug
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err("slug 格式无效");
    }
    if input.title.trim().is_empty() || input.title.chars().count() > 160 {
        return Err("标题长度无效");
    }
    if input
        .summary
        .as_ref()
        .is_some_and(|summary| summary.chars().count() > 500)
    {
        return Err("摘要过长");
    }
    render_document(&input.document).map_err(|_| "文章文档格式无效")
}

/// 校验分组与付费开关；试看由服务端正文生成，旧版手填试看字段不再参与授权。
async fn validate_access(
    state: &AppState,
    access: &ArticleAccessInput,
    html: &str,
) -> Result<(Option<Value>, Option<String>), &'static str> {
    if access.subscriber_only && access.column_public_id.is_none() {
        return Err("付费篇章必须属于专栏");
    }
    if let Some(id) = access.column_public_id {
        let exists = paid_column::Entity::find()
            .filter(paid_column::Column::PublicId.eq(id))
            .one(&state.db)
            .await
            .map_err(|_| "读取专栏失败")?;
        if exists.is_none() {
            return Err("专栏不存在");
        }
    }
    if !access.subscriber_only {
        return Ok((None, None));
    }
    Ok((
        None,
        Some(crate::domain::article::render::preview_html(html)),
    ))
}

/// 管理员可读取草稿与已发布文章。
#[utoipa::path(get, path = "/api/v1/admin/articles", params(AdminPageQuery), responses((status = 200, body = AdminArticlePage), (status = 401, body = ApiError)), tag = "admin")]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AdminPageQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let page = query.page.unwrap_or(1);
    if page == 0 || page > 100_000 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_pagination",
            "分页参数无效",
        );
    }
    let result = async {
        let mut articles = article::Entity::find();
        if let Some(column_id) = query.column_public_id {
            articles = articles.filter(article::Column::PaidColumnPublicId.eq(column_id));
        }
        let total = articles.clone().count(&state.db).await?;
        // 小册目录与公开页按发布时间排列；草稿排在后面，以创建时间稳定排序。
        let ordered = if query.column_public_id.is_some() {
            articles
                .order_by_asc(article::Column::PublishedAt)
                .order_by_asc(article::Column::CreatedAt)
                .order_by_asc(article::Column::PublicId)
        } else {
            articles.order_by_desc(article::Column::UpdatedAt)
        };
        let rows = ordered
            .limit(20)
            .offset((page - 1) * 20)
            .all(&state.db)
            .await?;
        Ok::<_, sea_orm::DbErr>((total, rows))
    }
    .await;
    match result {
        Ok((total, rows)) => Json(AdminArticlePage {
            items: rows.into_iter().map(Into::into).collect(),
            page,
            total,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "查询管理文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 创建草稿；发布需调用单独的状态接口。
#[utoipa::path(post, path = "/api/v1/admin/articles", request_body = ArticleInput, responses((status = 201, body = AdminArticleResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 409, body = ApiError)), tag = "admin")]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ArticleInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let html = match validate(&input) {
        Ok(html) => html,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_article", "文章内容无效"),
    };
    let access = input.access.clone().unwrap_or(ArticleAccessInput {
        column_public_id: None,
        subscriber_only: false,
    });
    let (preview_document, preview_html) = match validate_access(&state, &access, &html).await {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_article_access", message),
    };
    match article::Entity::find()
        .filter(article::Column::Slug.eq(&input.slug))
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "slug_conflict", "slug 已存在"),
        Err(err) => {
            tracing::error!(error = %err, "检查 slug 失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
        _ => {}
    }
    let now = Utc::now();
    let model = article::ActiveModel {
        id: NotSet,
        public_id: Set(Uuid::new_v4()),
        slug: Set(input.slug),
        title: Set(input.title.trim().to_owned()),
        summary: Set(input.summary),
        document: Set(input.document),
        rendered_html: Set(html),
        status: Set(ArticleStatus::Draft),
        cover_asset_id: Set(None),
        published_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        notion_page_id: Set(None),
        notion_last_edited_at: Set(None),
        notion_synced_at: Set(None),
        paid_column_public_id: Set(access.column_public_id),
        subscriber_only: Set(access.subscriber_only),
        preview_document: Set(preview_document),
        preview_html: Set(preview_html),
    };
    match model.insert(&state.db).await {
        Ok(row) => (StatusCode::CREATED, Json(AdminArticleResponse::from(row))).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "创建文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 管理员按 UUID 读取包含文档的文章。
#[utoipa::path(get, path = "/api/v1/admin/articles/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 200, body = AdminArticleResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match article::Entity::find()
        .filter(article::Column::PublicId.eq(public_id))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => Json(AdminArticleResponse::from(row)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取管理文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 更新正文并在同一事务中保存旧文档修订和搜索任务。
#[utoipa::path(put, path = "/api/v1/admin/articles/{public_id}", params(("public_id" = Uuid, Path)), request_body = ArticleInput, responses((status = 200, body = AdminArticleResponse), (status = 400, body = ApiError), (status = 404, body = ApiError)), tag = "admin")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<ArticleInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let html = match validate(&input) {
        Ok(html) => html,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_article", "文章内容无效"),
    };
    let access = if let Some(access) = &input.access {
        match validate_access(&state, access, &html).await {
            Ok((document, html)) => Some((access.clone(), document, html)),
            Err(message) => {
                return error(StatusCode::BAD_REQUEST, "invalid_article_access", message);
            }
        }
    } else {
        None
    };
    let result = async {
        let txn = state.db.begin().await?;
        let Some(old) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        if input.draft_only && old.status != ArticleStatus::Draft {
            return Err(sea_orm::DbErr::Custom("autosave_published".into()));
        }
        if input
            .expected_updated_at
            .is_some_and(|expected| expected != old.updated_at)
        {
            return Err(sea_orm::DbErr::Custom("article_version_conflict".into()));
        }
        let now = Utc::now();
        article_revision::ActiveModel {
            article_id: Set(old.id),
            document: Set(old.document.clone()),
            saved_at: Set(now),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        let mut active = old.clone().into_active_model();
        active.slug = Set(input.slug);
        active.title = Set(input.title.trim().to_owned());
        active.summary = Set(input.summary);
        active.document = Set(input.document);
        active.rendered_html = Set(html);
        if let Some((access, document, preview_html)) = access {
            active.paid_column_public_id = Set(access.column_public_id);
            active.subscriber_only = Set(access.subscriber_only);
            active.preview_document = Set(document);
            active.preview_html = Set(preview_html);
        }
        active.updated_at = Set(now);
        let row = active.update(&txn).await?;
        if row.status == ArticleStatus::Published {
            enqueue(&txn, row.id, SearchAction::Upsert, now).await?;
        }
        txn.commit().await?;
        Ok(Some(row))
    }
    .await;
    match result {
        Ok(Some(row)) => Json(AdminArticleResponse::from(row)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(sea_orm::DbErr::Custom(reason)) if reason == "autosave_published" => error(
            StatusCode::CONFLICT,
            "autosave_published",
            "文章已发布，自动保存不会修改公开正文，请刷新后核对",
        ),
        Err(sea_orm::DbErr::Custom(reason)) if reason == "article_version_conflict" => error(
            StatusCode::CONFLICT,
            "article_version_conflict",
            "服务器文章已被更新，已保留本地草稿，请刷新后核对",
        ),
        Err(err) => {
            tracing::error!(error = %err, "更新文章失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 发布或撤回文章，状态变更与搜索任务原子提交。
#[utoipa::path(post, path = "/api/v1/admin/articles/{public_id}/publish", params(("public_id" = Uuid, Path)), responses((status = 200, body = AdminArticleResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    change_status(state, headers, public_id, ArticleStatus::Published).await
}

/// 将文章撤回草稿，公开列表立即隐藏。
#[utoipa::path(post, path = "/api/v1/admin/articles/{public_id}/unpublish", params(("public_id" = Uuid, Path)), responses((status = 200, body = AdminArticleResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn unpublish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    change_status(state, headers, public_id, ArticleStatus::Draft).await
}

/// 统一执行状态变更和索引任务写入。
async fn change_status(
    state: AppState,
    headers: HeaderMap,
    public_id: Uuid,
    status: ArticleStatus,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(old) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let now = Utc::now();
        let action = if status == ArticleStatus::Published {
            SearchAction::Upsert
        } else {
            SearchAction::Delete
        };
        let mut active = old.into_active_model();
        active.status = Set(status);
        if status == ArticleStatus::Published && active.published_at.as_ref().is_none() {
            active.published_at = Set(Some(now));
        }
        active.updated_at = Set(now);
        let row = active.update(&txn).await?;
        enqueue(&txn, row.id, action, now).await?;
        txn.commit().await?;
        Ok(Some(row))
    }
    .await;
    match result {
        Ok(Some(row)) => Json(AdminArticleResponse::from(row)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "更改文章状态失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 在文章事务中写入可恢复的索引更新任务。
pub(super) async fn enqueue(
    txn: &sea_orm::DatabaseTransaction,
    article_id: i64,
    action: SearchAction,
    now: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    search_job::ActiveModel {
        article_id: Set(article_id),
        action: Set(action),
        status: Set(SearchJobStatus::Pending),
        attempts: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(txn)
    .await?;
    Ok(())
}

/// 查看最近 20 次修订，避免管理端加载无限历史。
#[utoipa::path(get, path = "/api/v1/admin/articles/{public_id}/revisions", params(("public_id" = Uuid, Path)), responses((status = 200, body = Vec<RevisionResponse>), (status = 404, body = ApiError)), tag = "admin")]
pub async fn revisions(
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
        let rows = article_revision::Entity::find()
            .filter(article_revision::Column::ArticleId.eq(row.id))
            .order_by_desc(article_revision::Column::SavedAt)
            .limit(20)
            .all(&state.db)
            .await?;
        Ok(Some(rows))
    }
    .await;
    match result {
        Ok(Some(rows)) => Json(
            rows.into_iter()
                .map(|row| RevisionResponse {
                    saved_at: row.saved_at,
                    document: row.document,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取修订失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除文章及其关联记录，同时保留搜索删除任务供索引 worker 消费。
#[utoipa::path(delete, path = "/api/v1/admin/articles/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 204), (status = 404, body = ApiError)), tag = "admin")]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = article::Entity::find()
            .filter(article::Column::PublicId.eq(public_id))
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(false);
        };
        comment::Entity::delete_many()
            .filter(comment::Column::ArticleId.eq(row.id))
            .filter(comment::Column::ParentId.is_not_null())
            .exec(&txn)
            .await?;
        comment::Entity::delete_many()
            .filter(comment::Column::ArticleId.eq(row.id))
            .exec(&txn)
            .await?;
        article_revision::Entity::delete_many()
            .filter(article_revision::Column::ArticleId.eq(row.id))
            .exec(&txn)
            .await?;
        article_category::Entity::delete_many()
            .filter(article_category::Column::ArticleId.eq(row.id))
            .exec(&txn)
            .await?;
        article_tag::Entity::delete_many()
            .filter(article_tag::Column::ArticleId.eq(row.id))
            .exec(&txn)
            .await?;
        article::Entity::delete_by_id(row.id).exec(&txn).await?;
        enqueue(&txn, row.id, SearchAction::Delete, Utc::now()).await?;
        txn.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "删除文章失败");
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
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_invalid_slug_and_unsafe_document() {
        let mut input = ArticleInput {
            expected_updated_at: None,
            draft_only: false,
            slug: "Hello!".into(),
            title: "标题".into(),
            summary: None,
            document: json!({"type":"doc"}),
            access: None,
        };
        assert!(validate(&input).is_err());
        input.slug = "hello".into();
        input.document = json!({"type":"doc","content":[{"type":"iframe"}]});
        assert!(validate(&input).is_err());
    }
}
