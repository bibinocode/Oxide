//! 管理员编辑器写作候选接口，不保存文章或修改选区。

use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use utoipa::ToSchema;

use crate::{
    agent::writing::{self, WritingAction},
    domain::agent::AgentTask,
    entity::agent_provider,
    infrastructure::agent as agent_runtime,
    web::{ApiError, AppState, error},
};

use super::super::auth::require_admin;

/// 浏览器传入的选区和有限上下文；无需文章已保存。
#[derive(Deserialize, ToSchema)]
pub struct WritingInput {
    /// 编辑器写作动作。
    pub action: WritingAction,
    /// 管理员补充的写作要求。
    pub instruction: String,
    /// 当前选中的 Markdown 源码。
    pub selected: String,
    /// 选区前的有限上下文。
    pub before: String,
    /// 选区后的有限上下文。
    pub after: String,
}

/// 待管理员核对差异后采纳的 Markdown 候选。
#[derive(Serialize, ToSchema)]
pub struct WritingResponse {
    /// 仅供审阅和采纳的候选内容。
    pub content: String,
}

/// SSE 的 JSON 载荷；`delta` 是增量，`done` 是校验后的完整候选。
#[derive(Serialize)]
struct WritingEvent<'a> {
    content: &'a str,
}

/// 统一验证输入并解析任务模型，供普通响应和流式响应复用。
async fn prepare(
    state: &AppState,
    input: &WritingInput,
) -> Result<(String, agent_provider::Model), Response> {
    let prompt = writing::prompt(
        input.action,
        &input.instruction,
        &input.selected,
        &input.before,
        &input.after,
    )
    .map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_writing_input",
            "写作输入无效或过长",
        )
    })?;
    let provider = match agent_runtime::text_provider(&state.db, AgentTask::Writing).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return Err(error(
                StatusCode::CONFLICT,
                "writing_model_unavailable",
                "请先在 AI 与 Agent 设置中绑定写作模型",
            ));
        }
        Err(cause) => {
            tracing::error!(error = %cause, "查询写作模型失败");
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            ));
        }
    };
    Ok((prompt, provider))
}

/// 调用 writing 绑定模型；服务端不会接受客户端传来的模型密钥。
#[utoipa::path(post, path = "/api/v1/admin/agent/writing", request_body = WritingInput, responses(
    (status = 200, body = WritingResponse), (status = 400, body = ApiError),
    (status = 401, body = ApiError), (status = 409, body = ApiError),
    (status = 502, body = ApiError)
), tag = "agent")]
pub async fn generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WritingInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let (prompt, provider) = match prepare(&state, &input).await {
        Ok(prepared) => prepared,
        Err(response) => return response,
    };
    match agent_runtime::generate_text(
        &provider,
        &state.comment_hash_key,
        writing::SYSTEM_PROMPT,
        prompt,
        writing::GENERATION_OPTIONS,
    )
    .await
    .and_then(|text| writing::validate_output(&text))
    {
        Ok(content) => Json(WritingResponse { content }).into_response(),
        Err(cause) => {
            tracing::warn!(provider = %provider.id, error = %cause, "文章写作生成失败");
            error(
                StatusCode::BAD_GATEWAY,
                "writing_generation_failed",
                "写作生成失败，请检查模型配置或稍后重试",
            )
        }
    }
}

/// 通过 POST 保留 CSRF 保护，浏览器用 fetch 读取 SSE 增量。
#[utoipa::path(post, path = "/api/v1/admin/agent/writing/stream", request_body = WritingInput, responses(
    (status = 200, content_type = "text/event-stream", body = String),
    (status = 400, body = ApiError), (status = 401, body = ApiError),
    (status = 409, body = ApiError)
), tag = "agent")]
pub async fn stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WritingInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let (prompt, provider) = match prepare(&state, &input).await {
        Ok(prepared) => prepared,
        Err(response) => return response,
    };
    let secret = state.comment_hash_key.clone();
    let (events_tx, events_rx) = mpsc::channel::<Result<Event, Infallible>>(32);
    tokio::spawn(async move {
        let (delta_tx, mut delta_rx) = mpsc::channel::<String>(32);
        let model = tokio::spawn(async move {
            agent_runtime::stream_text(
                &provider,
                &secret,
                writing::SYSTEM_PROMPT,
                prompt,
                writing::GENERATION_OPTIONS,
                delta_tx,
            )
            .await
        });
        while let Some(delta) = delta_rx.recv().await {
            let event = Event::default()
                .event("delta")
                .json_data(WritingEvent { content: &delta });
            if let Ok(event) = event
                && events_tx.send(Ok(event)).await.is_err()
            {
                model.abort();
                return;
            }
        }
        let outcome = model.await;
        let event = match outcome {
            Ok(Ok(text)) => match writing::validate_output(&text) {
                Ok(content) => Event::default()
                    .event("done")
                    .json_data(WritingEvent { content: &content }),
                Err(cause) => {
                    tracing::warn!(error = %cause, "写作流输出校验失败");
                    Event::default()
                        .event("error")
                        .json_data(serde_json::json!({"message": "生成内容无效，请重试"}))
                }
            },
            Ok(Err(cause)) => {
                tracing::warn!(error = %cause, "写作流调用失败");
                Event::default().event("error").json_data(
                    serde_json::json!({"message": "写作生成失败，请检查模型配置或稍后重试"}),
                )
            }
            Err(cause) => {
                tracing::error!(error = %cause, "写作流任务异常");
                Event::default()
                    .event("error")
                    .json_data(serde_json::json!({"message": "写作生成失败，请稍后重试"}))
            }
        };
        if let Ok(event) = event {
            let _ = events_tx.send(Ok(event)).await;
        }
    });
    Sse::new(ReceiverStream::new(events_rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}
