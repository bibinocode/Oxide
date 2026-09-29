//! 匿名评论提交、公开读取和稳定头像。

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::{
    article, comment,
    status::{ArticleStatus, CommentStatus},
};

use super::super::{ApiError, AppState, error};

/// 匿名评论输入；邮箱只参与摘要计算。
#[derive(Deserialize, ToSchema)]
pub struct CommentInput {
    /// 公开昵称。
    pub nickname: String,
    /// 邮箱；服务端不保存明文。
    pub email: String,
    /// 评论正文，以纯文本展示。
    pub body: String,
    /// 可选顶层评论 UUID，回复最多一层。
    pub parent_public_id: Option<Uuid>,
}

/// 提交后的审核状态。
#[derive(Serialize, ToSchema)]
pub struct CommentCreated {
    /// 新评论的 UUID。
    pub public_id: Uuid,
    /// 新评论进入待审核队列。
    pub status: &'static str,
}

/// 公开评论，不含邮箱或邮箱摘要。
#[derive(Serialize, ToSchema)]
pub struct CommentResponse {
    /// 评论 UUID。
    pub public_id: Uuid,
    /// 顶层评论 UUID，顶层评论为 null。
    pub parent_public_id: Option<Uuid>,
    /// 昵称。
    pub nickname: String,
    /// 纯文本正文。
    pub body: String,
    /// 稳定的站内头像路径。
    pub avatar_url: String,
    /// 提交时间。
    pub created_at: DateTime<Utc>,
}

/// 验证匿名输入并规范化邮箱。
fn validate(input: &CommentInput) -> Option<String> {
    let nickname = input.nickname.trim();
    let body = input.body.trim();
    let email = input.email.trim().to_lowercase();
    let (local, domain) = email.split_once('@')?;
    if nickname.is_empty()
        || nickname.chars().count() > 40
        || body.is_empty()
        || body.chars().count() > 2000
        || email.len() > 254
        || local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || email.chars().any(char::is_whitespace)
        || body.matches("http://").count() + body.matches("https://").count() > 3
    {
        return None;
    }
    Some(email)
}

/// 使用服务器密钥计算不可逆、稳定的头像标识。
fn email_hash(key: &str, email: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC 接受任意长度密钥");
    mac.update(email.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// 查询一篇已发布文章下的已审核评论。
#[utoipa::path(get, path = "/api/v1/articles/{slug}/comments", params(("slug" = String, Path)), responses((status = 200, body = Vec<CommentResponse>), (status = 404, body = ApiError)), tag = "comments")]
pub async fn list(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let result = async {
        let Some(article) = article::Entity::find()
            .filter(article::Column::Slug.eq(slug))
            .filter(article::Column::Status.eq(ArticleStatus::Published))
            .one(&state.db)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let rows = comment::Entity::find()
            .filter(comment::Column::ArticleId.eq(article.id))
            .filter(comment::Column::Status.eq(CommentStatus::Approved))
            .order_by_asc(comment::Column::CreatedAt)
            .all(&state.db)
            .await?;
        Ok(Some(rows))
    }
    .await;
    match result {
        Ok(Some(rows)) => {
            let parents: HashMap<i64, Uuid> =
                rows.iter().map(|row| (row.id, row.public_id)).collect();
            Json(
                rows.into_iter()
                    .map(|row| CommentResponse {
                        public_id: row.public_id,
                        parent_public_id: row.parent_id.and_then(|id| parents.get(&id).copied()),
                        nickname: row.nickname,
                        body: row.body,
                        avatar_url: format!("/api/v1/comments/{}/avatar.svg", row.public_id),
                        created_at: row.created_at,
                    })
                    .collect::<Vec<_>>(),
            )
            .into_response()
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取评论失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 匿名提交评论；Redis 不可用时拒绝提交，避免绕过限流。
#[utoipa::path(post, path = "/api/v1/articles/{slug}/comments", params(("slug" = String, Path)), request_body = CommentInput, responses((status = 202, body = CommentCreated), (status = 400, body = ApiError), (status = 429, body = ApiError)), tag = "comments")]
pub async fn create(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<CommentInput>,
) -> Response {
    let Some(email) = validate(&input) else {
        return error(StatusCode::BAD_REQUEST, "invalid_comment", "评论内容无效");
    };
    let article = match article::Entity::find()
        .filter(article::Column::Slug.eq(slug))
        .filter(article::Column::Status.eq(ArticleStatus::Published))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "article_not_found", "文章不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取评论文章失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let parent_id = if let Some(public_id) = input.parent_public_id {
        match comment::Entity::find()
            .filter(comment::Column::PublicId.eq(public_id))
            .filter(comment::Column::ArticleId.eq(article.id))
            .filter(comment::Column::Status.eq(CommentStatus::Approved))
            .one(&state.db)
            .await
        {
            Ok(Some(row)) if row.parent_id.is_none() => Some(row.id),
            Ok(_) => return error(StatusCode::BAD_REQUEST, "invalid_parent", "回复目标无效"),
            Err(err) => {
                tracing::error!(error = %err, "读取回复目标失败");
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
    let hash = email_hash(&state.comment_hash_key, &email);
    let mut redis = match state.redis.get_multiplexed_async_connection().await {
        Ok(redis) => redis,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "评论暂时不可用",
            );
        }
    };
    let key = format!("comment-rate:{}:{hash}", article.id);
    let count: i64 = match redis::cmd("INCR").arg(&key).query_async(&mut redis).await {
        Ok(count) => count,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "评论暂时不可用",
            );
        }
    };
    if count == 1
        && redis::cmd("EXPIRE")
            .arg(&key)
            .arg(3600)
            .query_async::<()>(&mut redis)
            .await
            .is_err()
    {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "评论暂时不可用",
        );
    }
    if count > 5 {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "comment_rate_limited",
            "提交过于频繁",
        );
    }
    let public_id = Uuid::new_v4();
    let model = comment::ActiveModel {
        id: NotSet,
        public_id: Set(public_id),
        article_id: Set(article.id),
        parent_id: Set(parent_id),
        nickname: Set(input.nickname.trim().into()),
        email_hash: Set(hash),
        body: Set(input.body.trim().into()),
        status: Set(CommentStatus::Pending),
        created_at: Set(Utc::now()),
        reviewed_at: Set(None),
    };
    match model.insert(&state.db).await {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(CommentCreated {
                public_id,
                status: "pending",
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "保存评论失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 只为已审核评论生成稳定的五列镜像几何头像。
#[utoipa::path(get, path = "/api/v1/comments/{public_id}/avatar.svg", params(("public_id" = Uuid, Path)), responses((status = 200, content_type = "image/svg+xml"), (status = 404, body = ApiError)), tag = "comments")]
pub async fn avatar(State(state): State<AppState>, Path(public_id): Path<Uuid>) -> Response {
    let row = match comment::Entity::find()
        .filter(comment::Column::PublicId.eq(public_id))
        .filter(comment::Column::Status.eq(CommentStatus::Approved))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "comment_not_found", "评论不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取头像失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let bytes = hex::decode(row.email_hash).unwrap_or_default();
    if bytes.len() < 18 {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "服务暂时不可用",
        );
    }
    let color = format!(
        "#{:02x}{:02x}{:02x}",
        bytes[0] / 2 + 64,
        bytes[1] / 2 + 64,
        bytes[2] / 2 + 64
    );
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><rect width=\"100\" height=\"100\" fill=\"#f2f4f1\"/>",
    );
    for y in 0..5 {
        for x in 0..3 {
            if bytes[3 + y * 3 + x] & 1 == 1 {
                let columns = [x, 4 - x];
                for column in columns.into_iter().take(if x == 2 { 1 } else { 2 }) {
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"20\" height=\"20\" fill=\"{color}\"/>",
                        column * 20,
                        y * 20
                    ));
                }
            }
        }
    }
    svg.push_str("</svg>");
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'none'",
            ),
        ],
        svg,
    )
        .into_response()
}
