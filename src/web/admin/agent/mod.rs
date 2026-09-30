//! Agent 模型注册、任务绑定与生成接口。

pub mod skills;
pub mod summary;
pub mod tools;
pub mod writing;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::super::{ApiError, AppState, error};
use super::auth::require_admin;
use crate::{
    domain::agent::AgentTask,
    entity::{agent_binding, agent_provider},
    infrastructure::secrets,
};

/// 写作与摘要共用的请求级能力加载边界，技能包和工具权限独立组合。
pub(super) async fn task_context(
    state: &AppState,
    task: AgentTask,
    base: &str,
    invocation: &str,
) -> Result<crate::agent::context::AgentContext, Response> {
    crate::agent::context::AgentContext::prepare(
        state.skills.clone(),
        state.tools.clone(),
        task,
        base,
        invocation,
    )
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent_context_unavailable",
            "Agent 技能或工具配置暂时不可用，请检查服务端配置",
        )
    })
}

/// 模型配置写入；更新时 API key 可留空以保留原密钥。
#[derive(Deserialize, ToSchema)]
pub struct ProviderInput {
    /// 稳定 ID。
    pub id: String,
    /// openai_compatible 或 jimeng。
    pub adapter: String,
    /// 后台显示名称。
    pub name: String,
    /// HTTPS API 基础地址。
    pub base_url: String,
    /// 模型 ID，由提供商决定。
    pub model_id: String,
    /// text 或 image。
    pub capability: String,
    /// 仅在写入时接收，读接口不会返回。
    pub api_key: Option<String>,
    /// 是否可被任务调用。
    pub enabled: bool,
}

/// 无密钥的模型配置响应。
#[derive(Serialize, ToSchema)]
pub struct ProviderResponse {
    pub id: String,
    pub adapter: String,
    pub name: String,
    pub base_url: String,
    pub model_id: String,
    pub capability: String,
    pub api_key_configured: bool,
    pub enabled: bool,
}

/// 任务绑定响应与写入格式。
#[derive(Serialize, Deserialize, ToSchema)]
pub struct BindingInput {
    /// 模型提供商 ID。
    pub provider_id: String,
}

/// 已配置的任务绑定。
#[derive(Serialize, ToSchema)]
pub struct BindingResponse {
    pub task: String,
    pub provider_id: String,
}

/// 限制模型基础地址和输入规模；协议类型需与模型能力匹配。
fn valid(input: &ProviderInput) -> bool {
    let url = url::Url::parse(&input.base_url).ok();
    let address_valid = url.is_some_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    });
    input.id.len() >= 2
        && input.id.len() <= 40
        && input
            .id
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        && !input.name.trim().is_empty()
        && input.name.len() <= 80
        && !input.model_id.trim().is_empty()
        && input.model_id.len() <= 120
        && matches!(input.adapter.as_str(), "openai_compatible" | "jimeng")
        && matches!(input.capability.as_str(), "text" | "image")
        && address_valid
        && input.api_key.as_ref().is_none_or(|key| key.len() <= 512)
}

/// 将持久化实体投影为不含任何密钥的管理响应。
fn response(row: agent_provider::Model) -> ProviderResponse {
    ProviderResponse {
        id: row.id,
        adapter: row.adapter,
        name: row.name,
        base_url: row.base_url,
        model_id: row.model_id,
        capability: row.capability,
        api_key_configured: !row.encrypted_api_key.is_empty(),
        enabled: row.enabled,
    }
}

/// 查询 Agent 模型提供商。
#[utoipa::path(get, path = "/api/v1/admin/agent/providers", responses((status = 200, body = Vec<ProviderResponse>), (status = 401, body = ApiError)), tag = "agent")]
pub async fn providers(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match agent_provider::Entity::find()
        .order_by_asc(agent_provider::Column::Name)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(rows.into_iter().map(response).collect::<Vec<_>>()).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取 Agent 模型失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 创建 Agent 模型配置。
#[utoipa::path(post, path = "/api/v1/admin/agent/providers", request_body = ProviderInput, responses((status = 201, body = ProviderResponse), (status = 400, body = ApiError)), tag = "agent")]
pub async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProviderInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid(&input) || input.api_key.as_ref().is_none_or(String::is_empty) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_agent_provider",
            "模型配置或 API Key 无效",
        );
    }
    let encrypted = match secrets::seal(
        input.api_key.as_deref().unwrap_or_default().as_bytes(),
        &state.comment_hash_key,
        "agent",
    ) {
        Ok(value) => value,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                "保存密钥失败",
            );
        }
    };
    let model = agent_provider::ActiveModel {
        id: Set(input.id),
        adapter: Set(input.adapter),
        name: Set(input.name.trim().into()),
        base_url: Set(input.base_url.trim_end_matches('/').into()),
        model_id: Set(input.model_id.trim().into()),
        capability: Set(input.capability),
        encrypted_api_key: Set(encrypted),
        enabled: Set(input.enabled),
        created_at: Set(Utc::now()),
    };
    match model.insert(&state.db).await {
        Ok(row) => (StatusCode::CREATED, Json(response(row))).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "创建 Agent 模型失败");
            error(
                StatusCode::CONFLICT,
                "agent_provider_conflict",
                "模型标识已存在",
            )
        }
    }
}

/// 更新模型与密钥；任务绑定继续引用相同 ID。
#[utoipa::path(put, path = "/api/v1/admin/agent/providers/{id}", params(("id" = String, Path)), request_body = ProviderInput, responses((status = 200, body = ProviderResponse), (status = 404, body = ApiError)), tag = "agent")]
pub async fn update_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid(&input) || id != input.id {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_agent_provider",
            "模型配置无效",
        );
    }
    let row = match agent_provider::Entity::find_by_id(&id).one(&state.db).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return error(
                StatusCode::NOT_FOUND,
                "agent_provider_not_found",
                "模型不存在",
            );
        }
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut active = row.into_active_model();
    active.adapter = Set(input.adapter);
    active.name = Set(input.name.trim().into());
    active.base_url = Set(input.base_url.trim_end_matches('/').into());
    active.model_id = Set(input.model_id.trim().into());
    active.capability = Set(input.capability);
    active.enabled = Set(input.enabled);
    if let Some(key) = input.api_key.filter(|key| !key.is_empty()) {
        match secrets::seal(key.as_bytes(), &state.comment_hash_key, "agent") {
            Ok(value) => active.encrypted_api_key = Set(value),
            Err(_) => {
                return error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "encryption_failed",
                    "保存密钥失败",
                );
            }
        }
    }
    match active.update(&state.db).await {
        Ok(row) => Json(response(row)).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "更新 Agent 模型失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 删除模型，同时删除其任务绑定。
#[utoipa::path(delete, path = "/api/v1/admin/agent/providers/{id}", params(("id" = String, Path)), responses((status = 204), (status = 404, body = ApiError)), tag = "agent")]
pub async fn delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match agent_provider::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
    {
        Ok(result) if result.rows_affected > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error(
            StatusCode::NOT_FOUND,
            "agent_provider_not_found",
            "模型不存在",
        ),
        Err(err) => {
            tracing::error!(error = %err, "删除 Agent 模型失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 查询写作、摘要、生图及对话的模型绑定。
#[utoipa::path(get, path = "/api/v1/admin/agent/bindings", responses((status = 200, body = Vec<BindingResponse>)), tag = "agent")]
pub async fn bindings(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match agent_binding::Entity::find()
        .order_by_asc(agent_binding::Column::Task)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(
            rows.into_iter()
                .map(|row| BindingResponse {
                    task: row.task,
                    provider_id: row.provider_id,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "读取 Agent 绑定失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 为任务绑定能力匹配且已启用的模型。
#[utoipa::path(put, path = "/api/v1/admin/agent/bindings/{task}", params(("task" = String, Path)), request_body = BindingInput, responses((status = 200, body = BindingResponse), (status = 400, body = ApiError)), tag = "agent")]
pub async fn bind_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task): Path<String>,
    Json(input): Json<BindingInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let Some(kind) = AgentTask::parse(&task) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_agent_task",
            "任务类型无效",
        );
    };
    let provider = match agent_provider::Entity::find_by_id(&input.provider_id)
        .one(&state.db)
        .await
    {
        Ok(Some(row)) if row.enabled && row.capability == kind.capability() => row,
        Ok(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_agent_provider",
                "模型不可用或能力不匹配",
            );
        }
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let model = agent_binding::ActiveModel {
        task: Set(task.clone()),
        provider_id: Set(provider.id.clone()),
    };
    let result = if agent_binding::Entity::find_by_id(&task)
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        model.update(&state.db).await
    } else {
        model.insert(&state.db).await
    };
    match result {
        Ok(_) => Json(BindingResponse {
            task,
            provider_id: provider.id,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "绑定 Agent 模型失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}
