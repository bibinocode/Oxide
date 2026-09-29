//! 管理员按当前编辑器内容请求候选摘要，不修改数据库中的文章。

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    agent::summary,
    domain::agent::AgentTask,
    infrastructure::agent as agent_runtime,
    web::{ApiError, AppState, error},
};

use super::super::auth::require_admin;

/// 当前未保存的编辑器内容，生成结果只返回给管理员确认。
#[derive(Deserialize, ToSchema)]
pub struct SummaryInput {
    pub title: String,
    pub source: String,
}

/// 可采纳的摘要候选值。
#[derive(Serialize, ToSchema)]
pub struct SummaryResponse {
    pub summary: String,
}

/// 调用已绑定的摘要模型；正文不进入日志，调用失败不影响现有摘要。
#[utoipa::path(post, path = "/api/v1/admin/agent/summary", request_body = SummaryInput, responses(
    (status = 200, body = SummaryResponse), (status = 400, body = ApiError),
    (status = 401, body = ApiError), (status = 409, body = ApiError),
    (status = 502, body = ApiError)
), tag = "agent")]
pub async fn generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SummaryInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let prompt = match summary::prompt(&input.title, &input.source) {
        Ok(prompt) => prompt,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_summary_input",
                "标题或正文无效",
            );
        }
    };
    let provider = match agent_runtime::text_provider(&state.db, AgentTask::Summary).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return error(
                StatusCode::CONFLICT,
                "summary_model_unavailable",
                "请先在 AI 与 Agent 设置中绑定摘要模型",
            );
        }
        Err(cause) => {
            tracing::error!(error = %cause, "查询摘要模型失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let generated = agent_runtime::generate_text(
        &provider,
        &state.comment_hash_key,
        summary::SYSTEM_PROMPT,
        prompt,
        summary::GENERATION_OPTIONS,
    )
    .await;
    match generated.and_then(|text| summary::validate_output(&text)) {
        Ok(summary) => Json(SummaryResponse { summary }).into_response(),
        Err(cause) => {
            tracing::warn!(provider = %provider.id, error = %cause, "生成文章摘要失败");
            error(
                StatusCode::BAD_GATEWAY,
                "summary_generation_failed",
                "摘要生成失败，请检查模型配置或稍后重试",
            )
        }
    }
}
