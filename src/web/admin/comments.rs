//! 评论审核与管理：人工决定优先，隐藏可恢复，删除顶层评论时同时处理回复。
use super::super::{ApiError, AppState, error};
use super::auth::require_admin;
use crate::{
    domain::agent::AgentTask,
    entity::{agent_binding, article, comment, status::CommentStatus},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// 管理员可查看状态、上下文及最近一次 Agent 审核依据；不输出邮箱摘要。
#[derive(Serialize, ToSchema)]
pub struct AdminCommentResponse {
    /// 评论公开标识。
    pub public_id: Uuid,
    /// 所属文章公开标识。
    pub article_public_id: Uuid,
    /// 所属文章标题，便于理解上下文。
    pub article_title: String,
    /// 读者公开昵称。
    pub nickname: String,
    /// 评论纯文本正文。
    pub body: String,
    /// 当前审核状态。
    pub status: &'static str,
    /// 提交时间。
    pub created_at: DateTime<Utc>,
    /// 最近人工或自动处理时间。
    pub reviewed_at: Option<DateTime<Utc>>,
    /// 最终处理来源，manual 或 agent。
    pub review_source: Option<String>,
    /// 最近模型审核依据和模型标识，人工修改后保留用于审计。
    pub agent_review: Option<serde_json::Value>,
}
/// 通过、拒绝和隐藏；隐藏仍保留正文，人工处理后不会被自动 Agent 重新放行。
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// 审核通过并公开。
    Approved,
    /// 拒绝公开，保留记录。
    Rejected,
    /// 人工隐藏，可再次通过恢复。
    Pending,
}
/// 评论的人工处理请求。
#[derive(Deserialize, ToSchema)]
pub struct ReviewInput {
    /// 人工目标状态，pending 表示隐藏。
    pub status: ReviewStatus,
}

/// 将数据库记录投影为可审计的管理状态。
fn response(row: comment::Model, article: &article::Model) -> AdminCommentResponse {
    AdminCommentResponse {
        public_id: row.public_id,
        article_public_id: article.public_id,
        article_title: article.title.clone(),
        nickname: row.nickname,
        body: row.body,
        status: match row.status {
            CommentStatus::Pending => "pending",
            CommentStatus::Approved => "approved",
            CommentStatus::Rejected => "rejected",
        },
        created_at: row.created_at,
        reviewed_at: row.reviewed_at,
        review_source: row.review_source,
        agent_review: row.agent_review,
    }
}

/// 评论分页及状态筛选；筛选在数据库执行，旧评论也能管理。
#[derive(Deserialize, utoipa::IntoParams)]
pub struct CommentPageQuery {
    /// 从 1 开始，每页 50 条。
    pub page: Option<u64>,
    /// all、pending、approved、rejected 或 hidden。
    pub status: Option<String>,
}

/// 分页包含待审、已处理和隐藏记录，人工可纠正任意审核状态。
#[utoipa::path(get, path = "/api/v1/admin/comments", params(CommentPageQuery), responses((status = 200, body = Vec<AdminCommentResponse>), (status = 401, body = ApiError)), tag = "admin")]
pub async fn admin_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CommentPageQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let page = query.page.unwrap_or(1);
    let filter = query.status.as_deref().unwrap_or("all");
    if page == 0
        || page > 100_000
        || !matches!(
            filter,
            "all" | "pending" | "approved" | "rejected" | "hidden"
        )
    {
        return error(StatusCode::BAD_REQUEST, "invalid_query", "评论分页参数无效");
    }
    let result = async {
        let mut selection = comment::Entity::find();
        match filter {
            "approved" => {
                selection = selection.filter(comment::Column::Status.eq(CommentStatus::Approved))
            }
            "rejected" => {
                selection = selection.filter(comment::Column::Status.eq(CommentStatus::Rejected))
            }
            "hidden" => {
                selection = selection
                    .filter(comment::Column::Status.eq(CommentStatus::Pending))
                    .filter(comment::Column::ReviewSource.eq("manual"))
            }
            "pending" => {
                selection = selection
                    .filter(comment::Column::Status.eq(CommentStatus::Pending))
                    .filter(
                        sea_orm::Condition::any()
                            .add(comment::Column::ReviewSource.is_null())
                            .add(comment::Column::ReviewSource.ne("manual")),
                    )
            }
            _ => {}
        }
        let rows = selection
            .order_by_desc(comment::Column::CreatedAt)
            .offset((page - 1) * 50)
            .limit(50)
            .all(&state.db)
            .await?;
        let articles = article::Entity::find()
            .filter(
                article::Column::Id.is_in(rows.iter().map(|r| r.article_id).collect::<Vec<_>>()),
            )
            .all(&state.db)
            .await?;
        Ok::<_, sea_orm::DbErr>((rows, articles))
    }
    .await;
    match result {
        Ok((rows, articles)) => {
            let map: HashMap<_, _> = articles.into_iter().map(|r| (r.id, r)).collect();
            Json(
                rows.into_iter()
                    .filter_map(|r| map.get(&r.article_id).map(|a| response(r, a)))
                    .collect::<Vec<_>>(),
            )
            .into_response()
        }
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "评论读取失败",
        ),
    }
}

/// 人工处理会递增版本，正在执行的模型不能覆盖本次状态。
#[utoipa::path(patch, path = "/api/v1/admin/comments/{public_id}", params(("public_id" = Uuid, Path)), request_body = ReviewInput, responses((status = 200, body = AdminCommentResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<ReviewInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = comment::Entity::find()
            .filter(comment::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let article = article::Entity::find_by_id(row.article_id)
            .one(&txn)
            .await?;
        let version = row.review_version;
        let mut active = row.into_active_model();
        active.status = Set(match input.status {
            ReviewStatus::Approved => CommentStatus::Approved,
            ReviewStatus::Rejected => CommentStatus::Rejected,
            ReviewStatus::Pending => CommentStatus::Pending,
        });
        active.reviewed_at = Set(Some(Utc::now()));
        active.review_source = Set(Some("manual".into()));
        active.review_version = Set(version + 1);
        let updated = active.update(&txn).await?;
        txn.commit().await?;
        Ok(article.map(|a| response(updated, &a)))
    }
    .await;
    match result {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "审核失败",
        ),
    }
}

/// 主动交给 Agent 重新审核；持久化待审状态，模型不可用时不冒充成功。
#[utoipa::path(post, path = "/api/v1/admin/comments/{public_id}/agent-review", params(("public_id" = Uuid, Path)), responses((status = 202, body = AdminCommentResponse), (status = 409, body = ApiError)), tag = "admin")]
pub async fn agent_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let enabled = agent_binding::Entity::find_by_id(AgentTask::CommentReview.as_str())
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .is_some()
        && crate::infrastructure::agent::text_provider(&state.db, AgentTask::CommentReview)
            .await
            .ok()
            .flatten()
            .is_some();
    if !enabled {
        return error(
            StatusCode::CONFLICT,
            "comment_agent_disabled",
            "请先在 Agent 模型设置中绑定评论审核模型",
        );
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = comment::Entity::find()
            .filter(comment::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let article = article::Entity::find_by_id(row.article_id)
            .one(&txn)
            .await?;
        let version = row.review_version;
        let mut active = row.into_active_model();
        active.status = Set(CommentStatus::Pending);
        active.reviewed_at = Set(None);
        active.review_source = Set(None);
        active.agent_review = Set(None);
        active.review_version = Set(version + 1);
        let updated = active.update(&txn).await?;
        txn.commit().await?;
        Ok(article.map(|a| response(updated, &a)))
    }
    .await;
    match result {
        Ok(Some(row)) => (StatusCode::ACCEPTED, Json(row)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "提交 Agent 审核失败",
        ),
    }
}

/// 删除顶层评论及其回复，或只删除选中的回复；同一事务避免半删和外键失败。
#[utoipa::path(delete, path = "/api/v1/admin/comments/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 204), (status = 404, body = ApiError)), tag = "admin")]
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
        let Some(row) = comment::Entity::find()
            .filter(comment::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(false);
        };
        comment::Entity::delete_many()
            .filter(comment::Column::ParentId.eq(row.id))
            .exec(&txn)
            .await?;
        comment::Entity::delete_by_id(row.id).exec(&txn).await?;
        txn.commit().await?;
        Ok(true)
    }
    .await;
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "评论删除失败",
        ),
    }
}
