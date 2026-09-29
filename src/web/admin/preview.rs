//! 编辑预览使用与正式保存完全相同的服务端渲染器。
use super::super::{ApiError, AppState, error};
use super::auth::require_admin;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// 仅包含正文，不要求发布属性。
#[derive(Deserialize, ToSchema)]
pub struct PreviewInput {
    /// Markdown 或兼容的 Tiptap 文档。
    pub document: Value,
}

/// 净化后的预览 HTML。
#[derive(Serialize, ToSchema)]
pub struct PreviewResponse {
    /// 与正式文章保存逻辑一致的 HTML。
    pub html: String,
}

/// 只渲染，不写入文章或修订。
#[utoipa::path(post, path = "/api/v1/admin/preview", request_body = PreviewInput, responses((status = 200, body = PreviewResponse), (status = 400, body = ApiError)), tag = "admin")]
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PreviewInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match crate::domain::article::render::render_document(&input.document) {
        Ok(html) => Json(PreviewResponse { html }).into_response(),
        Err(_) => error(
            StatusCode::BAD_REQUEST,
            "invalid_document",
            "正文无法渲染或超出大小限制",
        ),
    }
}
