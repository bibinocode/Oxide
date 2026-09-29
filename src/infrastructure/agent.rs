//! 从任务绑定解析文本模型，并通过 Rig 调用对应的提供商。

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use rig::{
    agent::{MultiTurnStreamItem, StreamingResult, Text},
    client::AgentClientExt,
    completion::Prompt,
    providers::{deepseek, openai},
    streaming::{StreamedAssistantContent, StreamingPrompt},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use tokio::sync::mpsc;

use crate::{
    agent::TextGenerationOptions,
    domain::agent::AgentTask,
    entity::{agent_binding, agent_provider},
    infrastructure::secrets,
};

/// 有绑定时严格使用绑定模型；只有一个可用文本模型时可作为初始默认值。
pub async fn text_provider(
    db: &DatabaseConnection,
    task: AgentTask,
) -> Result<Option<agent_provider::Model>> {
    let binding = agent_binding::Entity::find_by_id(task.as_str())
        .one(db)
        .await?;
    if let Some(binding) = binding {
        let provider = agent_provider::Entity::find_by_id(binding.provider_id)
            .one(db)
            .await?;
        return Ok(provider.filter(eligible_text_provider));
    }
    let mut providers = agent_provider::Entity::find()
        .filter(agent_provider::Column::Enabled.eq(true))
        .filter(agent_provider::Column::Capability.eq("text"))
        .filter(agent_provider::Column::Adapter.eq("openai_compatible"))
        .filter(agent_provider::Column::EncryptedApiKey.ne(""))
        .limit(2)
        .all(db)
        .await?;
    Ok(if providers.len() == 1 {
        providers.pop()
    } else {
        None
    })
}

fn eligible_text_provider(provider: &agent_provider::Model) -> bool {
    provider.enabled
        && provider.capability == "text"
        && provider.adapter == "openai_compatible"
        && !provider.encrypted_api_key.is_empty()
}

/// 将已加密的提供商配置交给 Rig Agent；不记录正文、摘要或密钥。
pub async fn generate_text(
    provider: &agent_provider::Model,
    server_key: &str,
    system_prompt: &str,
    user_prompt: String,
    options: TextGenerationOptions,
) -> Result<String> {
    let key = secrets::open(&provider.encrypted_api_key, server_key, "agent")?;
    let api_key = String::from_utf8(key).context("模型密钥编码无效")?;
    let host = url::Url::parse(&provider.base_url)?
        .host_str()
        .unwrap_or_default()
        .to_owned();
    let result = if host == "api.deepseek.com" {
        let client = deepseek::Client::builder()
            .api_key(api_key)
            .base_url(&provider.base_url)
            .build()?;
        let mut builder = client
            .agent(&provider.model_id)
            .preamble(system_prompt)
            .temperature(options.temperature)
            .max_tokens(options.max_tokens)
            .default_max_turns(1);
        if options.disable_reasoning {
            builder =
                builder.additional_params(serde_json::json!({"thinking": {"type": "disabled"}}));
        }
        let agent = builder.build();
        tokio::time::timeout(Duration::from_secs(45), agent.prompt(user_prompt)).await??
    } else {
        let client = openai::Client::builder()
            .api_key(api_key)
            .base_url(&provider.base_url)
            .build()?;
        let agent = client
            .completions_api()
            .agent(&provider.model_id)
            .preamble(system_prompt)
            .temperature(options.temperature)
            .max_tokens(options.max_tokens)
            .default_max_turns(1)
            .build();
        tokio::time::timeout(Duration::from_secs(45), agent.prompt(user_prompt)).await??
    };
    Ok(result)
}

/// 通过 Rig 接收真实模型增量；客户端断开后停止读取，完整结果交由任务层校验。
async fn forward_text_stream(
    mut stream: StreamingResult,
    sender: mpsc::Sender<String>,
) -> Result<String> {
    let mut result = String::new();
    while let Some(item) = stream.next().await {
        if let MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text {
            text,
            ..
        })) = item?
        {
            result.push_str(&text);
            if sender.send(text).await.is_err() {
                break;
            }
        }
    }
    Ok(result)
}

/// 根据提供商配置建立流，模型输出不写入日志。
pub async fn stream_text(
    provider: &agent_provider::Model,
    server_key: &str,
    system_prompt: &str,
    user_prompt: String,
    options: TextGenerationOptions,
    sender: mpsc::Sender<String>,
) -> Result<String> {
    let key = secrets::open(&provider.encrypted_api_key, server_key, "agent")?;
    let api_key = String::from_utf8(key).context("模型密钥编码无效")?;
    let host = url::Url::parse(&provider.base_url)?
        .host_str()
        .unwrap_or_default()
        .to_owned();
    let collect = async {
        if host == "api.deepseek.com" {
            let client = deepseek::Client::builder()
                .api_key(api_key)
                .base_url(&provider.base_url)
                .build()?;
            let mut builder = client
                .agent(&provider.model_id)
                .preamble(system_prompt)
                .temperature(options.temperature)
                .max_tokens(options.max_tokens)
                .default_max_turns(1);
            if options.disable_reasoning {
                builder = builder
                    .additional_params(serde_json::json!({"thinking": {"type": "disabled"}}));
            }
            let agent = builder.build();
            forward_text_stream(agent.stream_prompt(user_prompt).await, sender).await
        } else {
            let client = openai::Client::builder()
                .api_key(api_key)
                .base_url(&provider.base_url)
                .build()?;
            let agent = client
                .completions_api()
                .agent(&provider.model_id)
                .preamble(system_prompt)
                .temperature(options.temperature)
                .max_tokens(options.max_tokens)
                .default_max_turns(1)
                .build();
            forward_text_stream(agent.stream_prompt(user_prompt).await, sender).await
        }
    };
    tokio::time::timeout(Duration::from_secs(90), collect).await?
}
