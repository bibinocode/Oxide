//! 评论审核与管理接口。

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::{article, comment, status::CommentStatus};

use super::super::{ApiError, AppState, error};
use super::auth::require_admin;

/// 待审核列表项，仅增加文章 UUID 和状态。
#[derive(Serialize, ToSchema)]
pub struct AdminCommentResponse {
    /// 评论 UUID。
    pub public_id: Uuid,
    /// 文章 UUID。
    pub article_public_id: Uuid,
    /// 昵称。
    pub nickname: String,
    /// 正文。
    pub body: String,
    /// 审核状态。
    pub status: &'static str,
    /// 提交时间。
    pub created_at: DateTime<Utc>,
}

/// 审核请求，仅允许通过或拒绝。
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Approved,
    Rejected,
}

/// 评论审核请求体。
#[derive(Deserialize, ToSchema)]
pub struct ReviewInput {
    /// 新状态。
    pub status: ReviewStatus,
}

/// 管理端查看最近 100 条评论，含待审核和已处理记录。
#[utoipa::path(get, path = "/api/v1/admin/comments", responses((status = 200, body = Vec<AdminCommentResponse>), (status = 401, body = ApiError)), tag = "admin")]
pub async fn admin_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    let result = async {
        let rows = comment::Entity::find()
            .order_by_desc(comment::Column::CreatedAt)
            .limit(100)
            .all(&state.db)
            .await?;
        let ids: Vec<i64> = rows.iter().map(|row| row.article_id).collect();
        let articles = article::Entity::find()
            .filter(article::Column::Id.is_in(ids))
            .all(&state.db)
            .await?;
        Ok::<_, sea_orm::DbErr>((rows, articles))
    }
    .await;
    match result {
        Ok((rows, articles)) => {
            let ids: HashMap<i64, Uuid> = articles
                .into_iter()
                .map(|row| (row.id, row.public_id))
                .collect();
            Json(
                rows.into_iter()
                    .filter_map(|row| {
                        Some(AdminCommentResponse {
                            public_id: row.public_id,
                            article_public_id: *ids.get(&row.article_id)?,
                            nickname: row.nickname,
                            body: row.body,
                            status: match row.status {
                                CommentStatus::Pending => "pending",
                                CommentStatus::Approved => "approved",
                                CommentStatus::Rejected => "rejected",
                            },
                            created_at: row.created_at,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .into_response()
        }
        Err(err) => {
            tracing::error!(error = %err, "读取管理评论失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 审核评论，客户端只能选择通过或拒绝。
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
        let Some(row) = comment::Entity::find()
            .filter(comment::Column::PublicId.eq(public_id))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let article = article::Entity::find_by_id(row.article_id)
            .one(&state.db)
            .await?;
        let mut active = row.into_active_model();
        active.status = Set(match input.status {
            ReviewStatus::Approved => CommentStatus::Approved,
            ReviewStatus::Rejected => CommentStatus::Rejected,
        });
        active.reviewed_at = Set(Some(Utc::now()));
        let updated = active.update(&state.db).await?;
        Ok(article.map(|article| (updated, article)))
    }
    .await;
    match result {
        Ok(Some((row, article))) => Json(AdminCommentResponse {
            public_id: row.public_id,
            article_public_id: article.public_id,
            nickname: row.nickname,
            body: row.body,
            status: if row.status == CommentStatus::Approved {
                "approved"
            } else {
                "rejected"
            },
            created_at: row.created_at,
        })
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(err) => {
            tracing::error!(error = %err, "审核评论失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除评论；有回复时由外键策略决定是否可删除。
#[utoipa::path(delete, path = "/api/v1/admin/comments/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 204), (status = 404, body = ApiError)), tag = "admin")]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match comment::Entity::delete_many()
        .filter(comment::Column::PublicId.eq(public_id))
        .exec(&state.db)
        .await
    {
        Ok(result) if result.rows_affected > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(err) => {
            tracing::error!(error = %err, "删除评论失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}
