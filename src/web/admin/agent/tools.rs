//! 工具管理 API：不接收密钥，读取环境配置状态，所有写操作验证 CSRF。

use super::super::auth::require_admin;
use crate::{
    domain::agent_tool::WebSearchSettings,
    infrastructure::web_search::{SearchError, SearchInput, SearchResponse},
    web::{ApiError, AppState},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

/// 将搜索故障转换为可恢复的 HTTP 错误，绝不回传原始提供商响应。
fn search_error(cause: SearchError) -> Response {
    let (status, code) = match &cause {
        SearchError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_search_input"),
        SearchError::NotConfigured | SearchError::Disabled => {
            (StatusCode::CONFLICT, "web_search_unavailable")
        }
        SearchError::Busy => (StatusCode::TOO_MANY_REQUESTS, "web_search_busy"),
        SearchError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "web_search_timeout"),
        SearchError::Configuration => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "tool_configuration_error",
        ),
        _ => (StatusCode::BAD_GATEWAY, "web_search_failed"),
    };
    (
        status,
        Json(serde_json::json!({"code": code, "message": cause.to_string()})),
    )
        .into_response()
}

/// 工具注册列表包含可用性与参数 Schema，不包含密钥或密钥片段。
#[utoipa::path(get, path = "/api/v1/admin/agent/tools", responses((status = 200, body = Vec<crate::agent::tools::ToolDescriptor>), (status = 401, body = ApiError)), tag = "agent")]
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match state.tools.list().await {
        Ok(tools) => Json(tools).into_response(),
        Err(_) => search_error(SearchError::Configuration),
    }
}

/// 保存启停、任务授权和非敏感搜索参数，密钥继续由运行环境管理。
#[utoipa::path(put, path = "/api/v1/admin/agent/tools/webSearch", request_body = WebSearchSettings, responses((status = 200, body = WebSearchSettings), (status = 400, body = ApiError), (status = 403, body = ApiError)), tag = "agent")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WebSearchSettings>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let settings = match input.validated() {
        Ok(settings) => settings,
        Err(cause) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"code": "invalid_tool_configuration", "message": cause.to_string()}))).into_response(),
    };
    match state.tools.save(settings).await {
        Ok(settings) => Json(settings).into_response(),
        Err(_) => search_error(SearchError::Configuration),
    }
}

/// 管理员主动发起一次真实搜索，使用已保存的设置，不调用聊天模型。
#[utoipa::path(post, path = "/api/v1/admin/agent/tools/webSearch/test", request_body = SearchInput, responses((status = 200, body = SearchResponse), (status = 400, body = ApiError), (status = 409, body = ApiError), (status = 502, body = ApiError)), tag = "agent")]
pub async fn test(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SearchInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match state.tools.test(input).await {
        Ok(response) => Json(response).into_response(),
        Err(cause) => search_error(cause),
    }
}
