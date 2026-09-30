//! 从任务绑定解析文本模型，并通过 Rig 调用对应的提供商。

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use rig::{
    agent::{MultiTurnStreamItem, StreamingResult, Text},
    client::AgentClientExt,
    completion::{Message, Prompt},
    providers::{deepseek, openai},
    streaming::{StreamedAssistantContent, StreamingChat},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde::Serialize;
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

/// 模型过程与正文分离，前端不得将思考或工具参数插入文章。
#[derive(Serialize)]
pub struct TextStreamEvent {
    /// SSE 事件类型由适配器决定，不允许提供商覆盖。
    #[serde(skip)]
    kind: &'static str,
    pub id: String,
    pub content: String,
    pub delta: bool,
}

impl TextStreamEvent {
    /// 返回固定的 SSE 事件名，正文和过程不能互相冒充。
    pub fn event_name(&self) -> &'static str {
        self.kind
    }
}

/// 仅转发真实模型事件；正文和过程各自受限，客户端断开后停止读取。
async fn forward_text_stream(
    mut stream: StreamingResult,
    sender: mpsc::Sender<TextStreamEvent>,
) -> Result<String> {
    let mut result = String::new();
    let mut progress_bytes = 0usize;
    while let Some(item) = stream.next().await {
        let event = match item? {
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text {
                text,
                ..
            })) => {
                result.push_str(&text);
                if result.len() > crate::agent::writing::MAX_OUTPUT_CHARS * 4 {
                    anyhow::bail!("写作流输出超过上限");
                }
                TextStreamEvent {
                    kind: "delta",
                    id: "text".into(),
                    content: text,
                    delta: true,
                }
            }
            MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta { id, reasoning, .. },
            ) => TextStreamEvent {
                kind: "reasoning",
                id,
                content: reasoning,
                delta: true,
            },
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning {
                id,
                reasoning,
            }) => {
                let content = reasoning
                    .content
                    .iter()
                    .filter_map(|part| match part {
                        rig::message::ReasoningContent::Text { text, .. } => Some(text.as_str()),
                        rig::message::ReasoningContent::Summary(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                TextStreamEvent {
                    kind: "reasoning",
                    id,
                    content,
                    delta: false,
                }
            }
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                internal_call_id,
            }) => TextStreamEvent {
                kind: "tool",
                id: internal_call_id,
                content: format!("模型请求工具：{}", tool_call.function.name),
                delta: false,
            },
            _ => continue,
        };
        if event.kind != "delta" {
            progress_bytes += event.content.len();
            if progress_bytes > 160_000 {
                continue;
            }
        }
        if sender.send(event).await.is_err() {
            break;
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
    sender: mpsc::Sender<TextStreamEvent>,
    history: Vec<Message>,
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
            forward_text_stream(agent.stream_chat(user_prompt, history).await, sender).await
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
            forward_text_stream(agent.stream_chat(user_prompt, history).await, sender).await
        }
    };
    tokio::time::timeout(Duration::from_secs(180), collect).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 思考增量及最终替换独立发送，聚合正文不混入过程文本。
    #[tokio::test]
    async fn separates_reasoning_from_document_content() {
        let events = vec![
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta {
                    id: "reason-1".into(),
                    provider_id: None,
                    reasoning: "过程增量".into(),
                },
            )),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Reasoning {
                    id: "reason-1".into(),
                    reasoning: rig::message::Reasoning::new("完整过程"),
                },
            )),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::text("正文"),
            )),
        ];
        let (sender, mut receiver) = mpsc::channel(8);
        let result = forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
            .await
            .unwrap();
        assert_eq!(result, "正文");
        let delta = receiver.recv().await.unwrap();
        assert_eq!(delta.event_name(), "reasoning");
        assert!(delta.delta);
        let replacement = receiver.recv().await.unwrap();
        assert_eq!(replacement.id, delta.id);
        assert!(!replacement.delta);
        assert_eq!(replacement.content, "完整过程");
        assert_eq!(receiver.recv().await.unwrap().event_name(), "delta");
    }

    /// 加密推理载荷不可被当成可见思考文本泄露。
    #[tokio::test]
    async fn excludes_encrypted_reasoning_payload() {
        let events = vec![Ok(MultiTurnStreamItem::StreamAssistantItem(
            StreamedAssistantContent::Reasoning {
                id: "encrypted".into(),
                reasoning: rig::message::Reasoning::encrypted("opaque-secret"),
            },
        ))];
        let (sender, mut receiver) = mpsc::channel(8);
        assert!(
            forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(receiver.recv().await.unwrap().content.is_empty());
    }

    /// 客户端已关闭时不继续消费后续模型增量。
    #[tokio::test]
    async fn stops_reading_after_receiver_closes() {
        let events = vec![
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::text("第一段"),
            )),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::text("第二段"),
            )),
        ];
        let (sender, receiver) = mpsc::channel(8);
        drop(receiver);
        assert_eq!(
            forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
                .await
                .unwrap(),
            "第一段"
        );
    }
}
