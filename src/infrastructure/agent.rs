//! 从任务绑定解析文本模型，并通过 Rig 调用对应的提供商。

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use rig::{
    agent::{MultiTurnStreamItem, StreamingResult, Text},
    client::AgentClientExt,
    completion::{Message, Prompt},
    providers::{deepseek, openai},
    streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat},
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
    skills: &crate::agent::context::AgentContext,
    user_prompt: String,
    options: TextGenerationOptions,
) -> Result<String> {
    generate_message(
        provider,
        server_key,
        skills,
        Message::user(user_prompt),
        options,
    )
    .await
}

/// Rig 多模态入口；正文、图片与模型密钥都不写入日志。
pub async fn generate_message(
    provider: &agent_provider::Model,
    server_key: &str,
    skills: &crate::agent::context::AgentContext,
    user_prompt: Message,
    options: TextGenerationOptions,
) -> Result<String> {
    let key = secrets::open(&provider.encrypted_api_key, server_key, "agent")?;
    let api_key = String::from_utf8(key).context("模型密钥编码无效")?;
    let timeout = Duration::from_secs(if skills.has_tools() { 180 } else { 45 });
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
            .preamble(&skills.system)
            .temperature(options.temperature)
            .max_tokens(options.max_tokens)
            .default_max_turns(1);
        if options.disable_reasoning {
            builder =
                builder.additional_params(serde_json::json!({"thinking": {"type": "disabled"}}));
        }
        let agent = skills.build_agent(builder);
        tokio::time::timeout(timeout, agent.prompt(user_prompt)).await??
    } else {
        let client = openai::Client::builder()
            .api_key(api_key)
            .base_url(&provider.base_url)
            .build()?;
        let builder = client
            .completions_api()
            .agent(&provider.model_id)
            .preamble(&skills.system)
            .temperature(options.temperature)
            .max_tokens(options.max_tokens)
            .default_max_turns(1);
        let agent = skills.build_agent(builder);
        tokio::time::timeout(timeout, agent.prompt(user_prompt)).await??
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
    /// 真实搜索结果的来源索引，与最终 Markdown 和思考过程分开传输。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<super::web_search::SearchSource>,
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
                    sources: Vec::new(),
                }
            }
            MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta { id, reasoning, .. },
            ) => TextStreamEvent {
                kind: "reasoning",
                id,
                content: reasoning,
                delta: true,
                sources: Vec::new(),
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
                    sources: Vec::new(),
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
                sources: Vec::new(),
            },
            MultiTurnStreamItem::ToolExecutionCommitted {
                tool_call,
                internal_call_id,
            } => TextStreamEvent {
                kind: "tool",
                id: internal_call_id,
                content: format!("工具执行中：{}", tool_call.function.name),
                delta: false,
                sources: Vec::new(),
            },
            MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
                tool_result,
                internal_call_id,
            }) => {
                let value = tool_result.content.iter().find_map(|part| {
                    part.as_json().cloned().or_else(|| {
                        part.as_text()
                            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                    })
                });
                let content = if tool_result.name == "webfetch" {
                    match value.as_ref() {
                        Some(value) if value["ok"] == true => format!(
                            "网页读取完成：{}{}",
                            value["page"]["title"].as_str().unwrap_or("文章"),
                            if value["page"]["truncated"] == true {
                                "（正文已截断）"
                            } else {
                                ""
                            }
                        ),
                        Some(value) if value["ok"] == false => format!(
                            "网页读取失败：{}",
                            value["error"].as_str().unwrap_or("网页不可用")
                        ),
                        _ => "网页读取结果不可用".into(),
                    }
                } else if tool_result.name == "webSearch" {
                    match value.as_ref() {
                        Some(value) if value["ok"] == true => format!(
                            "网络搜索完成：找到 {} 条来源",
                            value["search"]["results"].as_array().map_or(0, Vec::len)
                        ),
                        Some(value) if value["ok"] == false => format!(
                            "网络搜索失败：{}",
                            value["error"].as_str().unwrap_or("搜索不可用")
                        ),
                        _ => "网络搜索返回结果不可用".into(),
                    }
                } else {
                    format!("工具执行完成：{}", tool_result.name)
                };
                let sources = if tool_result.name == "webfetch"
                    && value.as_ref().is_some_and(|value| value["ok"] == true)
                {
                    value
                        .as_ref()
                        .and_then(|value| {
                            let page = &value["page"];
                            let url = page["url"].as_str()?;
                            Some(vec![super::web_search::SearchSource {
                                reference: value["reference"].as_str()?.to_owned(),
                                title: page["title"].as_str().unwrap_or("文章").to_owned(),
                                url: url.to_owned(),
                                source: url::Url::parse(url).ok()?.host_str()?.to_owned(),
                                publish_date: None,
                            }])
                        })
                        .unwrap_or_default()
                } else if tool_result.name == "webSearch"
                    && value.as_ref().is_some_and(|value| value["ok"] == true)
                {
                    value
                        .as_ref()
                        .and_then(|value| value["search"]["results"].as_array())
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|row| serde_json::from_value(row.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                TextStreamEvent {
                    kind: "tool",
                    id: internal_call_id,
                    content,
                    delta: false,
                    sources,
                }
            }
            MultiTurnStreamItem::FinalResponse(response) => {
                result = response.output;
                continue;
            }
            _ => continue,
        };
        if event.kind != "delta" {
            progress_bytes += event.content.len();
            progress_bytes += event
                .sources
                .iter()
                .map(|source| source.url.len() + source.title.len() + source.source.len())
                .sum::<usize>();
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
    skills: &crate::agent::context::AgentContext,
    user_prompt: Message,
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
                .preamble(&skills.system)
                .temperature(options.temperature)
                .max_tokens(options.max_tokens)
                .default_max_turns(1);
            if options.disable_reasoning {
                builder = builder
                    .additional_params(serde_json::json!({"thinking": {"type": "disabled"}}));
            }
            let agent = skills.build_agent(builder);
            forward_text_stream(agent.stream_chat(user_prompt, history).await, sender).await
        } else {
            let client = openai::Client::builder()
                .api_key(api_key)
                .base_url(&provider.base_url)
                .build()?;
            let builder = client
                .completions_api()
                .agent(&provider.model_id)
                .preamble(&skills.system)
                .temperature(options.temperature)
                .max_tokens(options.max_tokens)
                .default_max_turns(1);
            let agent = skills.build_agent(builder);
            forward_text_stream(agent.stream_chat(user_prompt, history).await, sender).await
        }
    };
    tokio::time::timeout(Duration::from_secs(180), collect).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 未安装任何 Skill 时仍能注册搜索、执行真实工具协议并把来源带入下一轮模型。
    #[tokio::test]
    async fn web_search_runs_without_skills_and_respects_task_scope() {
        use crate::{
            agent::{context::AgentContext, tools::ToolRegistry},
            infrastructure::{agent_skills::SkillStore, web_search::SearchClient},
        };
        use axum::{Json, Router, routing::post};
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("oxide-search-loop-{}", uuid::Uuid::new_v4()));
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::<serde_json::Value>::new()));
        let observed = requests.clone();
        let app = Router::new()
            .route("/search", post(|Json(input): Json<serde_json::Value>| async move {
                assert_eq!(input["search_query"], "Rust 文档");
                Json(serde_json::json!({"search_result": [{"title": "Rust", "content": "官方文档摘要", "link": "https://www.rust-lang.org/"}]}))
            }))
            .route("/v1/chat/completions", post(move |Json(input): Json<serde_json::Value>| {
                let observed = observed.clone();
                async move {
                    let mut requests = observed.lock().await;
                    let first = requests.is_empty(); requests.push(input);
                    let (message, finish) = if first {
                        (serde_json::json!({"role": "assistant", "content": null, "tool_calls": [{"id": "search-call", "type": "function", "function": {"name": "webSearch", "arguments": "{\"query\":\"Rust 文档\"}"}}]}), "tool_calls")
                    } else { (serde_json::json!({"role": "assistant", "content": "参考官方来源完成回答"}), "stop") };
                    Json(serde_json::json!({"id": "mock", "object": "chat.completion", "created": 0, "model": "mock", "choices": [{"index": 0, "message": message, "finish_reason": finish}], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}))
                }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let registry = Arc::new(ToolRegistry::with_search(
            root.join("tools.json"),
            SearchClient::mock(format!("http://{address}/search")),
        ));
        let store = Arc::new(SkillStore::new(root.join("skills")));
        let context = AgentContext::prepare(
            store.clone(),
            registry.clone(),
            AgentTask::Writing,
            "基础约束",
            "",
        )
        .await
        .unwrap();
        assert!(context.has_tools());
        let summary = AgentContext::prepare(
            store.clone(),
            registry.clone(),
            AgentTask::Summary,
            "基础约束",
            "",
        )
        .await
        .unwrap();
        assert!(!summary.has_tools());
        let secret = "test-server-key-with-at-least-32-bytes";
        let provider = agent_provider::Model {
            id: "MOCK".into(),
            adapter: "openai_compatible".into(),
            name: "Mock".into(),
            base_url: format!("http://{address}/v1"),
            model_id: "mock".into(),
            capability: "text".into(),
            encrypted_api_key: secrets::seal(b"mock-model-key", secret, "agent").unwrap(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let result = generate_text(
            &provider,
            secret,
            &context,
            "搜索 Rust 文档".into(),
            crate::agent::summary::GENERATION_OPTIONS,
        )
        .await;
        let offline = generate_text(
            &provider,
            secret,
            &context.without_web_search(),
            "本轮不联网".into(),
            crate::agent::summary::GENERATION_OPTIONS,
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), "参考官方来源完成回答");
        assert!(offline.is_ok());
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);
        assert!(
            requests[2]
                .get("tools")
                .and_then(serde_json::Value::as_array)
                .is_none_or(Vec::is_empty)
        );
        assert!(
            requests[2]["messages"]
                .to_string()
                .contains("本轮禁止联网搜索")
        );
        assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 2);
        assert_eq!(requests[0]["tools"][0]["function"]["name"], "webSearch");
        assert_eq!(requests[0]["tools"][1]["function"]["name"], "webfetch");
        assert!(
            requests[1]["messages"]
                .to_string()
                .contains("https://www.rust-lang.org/")
        );
        assert!(!requests[1].to_string().contains("test-search-secret"));
    }

    /// 本地模拟 OpenAI 三轮响应，验证技能发现、核心加载和资源加载，不访问真实模型。
    #[tokio::test]
    async fn rig_keeps_inline_images_in_the_user_message() {
        use crate::agent::writing_image::{WritingImage, user_message};
        use axum::{Json, Router, routing::post};
        let observed = std::sync::Arc::new(tokio::sync::Mutex::new(None::<serde_json::Value>));
        let captured = observed.clone();
        let app = Router::new().route("/v1/chat/completions", post(move |Json(input): Json<serde_json::Value>| {
            let captured = captured.clone();
            async move { *captured.lock().await = Some(input); Json(serde_json::json!({"id": "mock", "object": "chat.completion", "created": 0, "model": "mock", "choices": [{"index": 0, "message": {"role": "assistant", "content": "这是图片"}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}})) }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let secret = "image-mock-key-with-at-least-32-bytes";
        let provider = agent_provider::Model {
            id: "MOCK".into(),
            adapter: "openai_compatible".into(),
            name: "Mock".into(),
            base_url: format!("http://{address}/v1"),
            model_id: "mock".into(),
            capability: "text".into(),
            encrypted_api_key: secrets::seal(b"mock-key", secret, "agent").unwrap(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let root = std::env::temp_dir().join(format!("oxide-image-mock-{}", uuid::Uuid::new_v4()));
        let skills = crate::agent::skills::SkillContext::discover(
            std::sync::Arc::new(crate::infrastructure::agent_skills::SkillStore::new(root)),
            "基础约束",
            "",
        )
        .await
        .unwrap();
        let context = crate::agent::context::AgentContext::skills_only(skills);
        let image = WritingImage { name: "pixel.png".into(), mime_type: "image/png".into(), data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aL1sAAAAASUVORK5CYII=".into() };
        let result = generate_message(
            &provider,
            secret,
            &context,
            user_message("描述图片".into(), vec![image]),
            crate::agent::summary::GENERATION_OPTIONS,
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), "这是图片");
        let captured = observed.lock().await;
        let body = captured.as_ref().unwrap();
        assert_eq!(body["messages"][1]["content"][0]["type"], "text");
        assert_eq!(body["messages"][1]["content"][1]["type"], "image_url");
        assert!(
            body["messages"][1]["content"][1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }

    /// 搜索来源单独传输，不依赖模型是否在正文正确引用。
    #[tokio::test]
    async fn search_sources_are_structured_progress() {
        let events = vec![Ok(MultiTurnStreamItem::StreamUserItem(
            StreamedUserContent::ToolResult {
                tool_result: rig::message::ToolResult {
                    call: rig::message::ToolCallId::new("search-call").unwrap(),
                    provider: None,
                    name: "webSearch".into(),
                    content: vec![rig::message::ToolResultContent::Json {
                        value: serde_json::json!({"ok": true, "search": {"results": [{"reference": "S1", "title": "Rust", "url": "https://www.rust-lang.org/", "source": "Rust", "publish_date": null, "content": "不应混入来源事件的摘要"}]}}),
                    }],
                },
                internal_call_id: "source-call".into(),
            },
        ))];
        let (sender, mut receiver) = mpsc::channel(8);
        assert_eq!(
            forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
                .await
                .unwrap(),
            ""
        );
        let event = receiver.recv().await.unwrap();
        assert_eq!(event.sources.len(), 1);
        assert_eq!(event.sources[0].reference, "S1");
        assert!(!serde_json::to_string(&event).unwrap().contains("不应混入"));
    }

    /// 本地模拟 OpenAI 三轮响应，验证技能发现、核心加载和资源加载，不访问真实模型。
    #[tokio::test]
    async fn model_uses_progressive_skill_loading() {
        use crate::{
            agent::skills::SkillContext,
            infrastructure::agent_skills::{PackageFile, SkillStore},
        };
        use axum::{Json, Router, routing::post};
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("oxide-skill-model-{}", uuid::Uuid::new_v4()));
        let store = Arc::new(SkillStore::new(root.clone()));
        store.install_files(vec![
            PackageFile { path: "SKILL.md".into(), bytes: b"---\nname: test-skill\ndescription: Use for testing\n---\nCORE_INSTRUCTIONS: read references/info.md".into(), mode: None },
            PackageFile { path: "references/info.md".into(), bytes: b"RESOURCE_DETAILS".into(), mode: None },
        ], String::new(), false).await.unwrap();
        store.set_enabled("test-skill", true).await.unwrap();
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::<serde_json::Value>::new()));
        let observed = requests.clone();
        let app = Router::new().route("/v1/chat/completions", post(move |Json(input): Json<serde_json::Value>| {
            let observed = observed.clone();
            async move {
                let mut requests = observed.lock().await;
                let step = requests.len(); requests.push(input);
                let (message, finish) = if step < 2 {
                    (serde_json::json!({"role": "assistant", "content": null, "tool_calls": [{"id": format!("call-{step}"), "type": "function", "function": {"name": "read_skill_file", "arguments": serde_json::json!({"skill": "test-skill", "path": if step == 0 { "SKILL.md" } else { "references/info.md" }}).to_string()}}]}), "tool_calls")
                } else { (serde_json::json!({"role": "assistant", "content": "最终正文"}), "stop") };
                Json(serde_json::json!({"id": "mock", "object": "chat.completion", "created": 0, "model": "mock", "choices": [{"index": 0, "message": message, "finish_reason": finish}], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let secret = "local-test-key-with-at-least-32-bytes";
        let provider = agent_provider::Model {
            id: "MOCK".into(),
            adapter: "openai_compatible".into(),
            name: "Mock".into(),
            base_url: format!("http://{address}/v1"),
            model_id: "mock".into(),
            capability: "text".into(),
            encrypted_api_key: secrets::seal(b"mock-api-key", secret, "agent").unwrap(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let context = SkillContext::discover(store.clone(), "基础约束", "测试技能")
            .await
            .unwrap();
        let context = crate::agent::context::AgentContext::skills_only(context);
        let result = generate_text(
            &provider,
            secret,
            &context,
            "test".into(),
            crate::agent::summary::GENERATION_OPTIONS,
        )
        .await;
        server.abort();
        store.uninstall("test-skill").await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(result.unwrap(), "最终正文");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);
        let initial = requests[0]["messages"].to_string();
        assert!(initial.contains("Use for testing"));
        assert!(!initial.contains("CORE_INSTRUCTIONS"));
        assert!(!initial.contains("RESOURCE_DETAILS"));
        assert!(
            requests[1]["messages"]
                .to_string()
                .contains("CORE_INSTRUCTIONS")
        );
        assert!(
            requests[2]["messages"]
                .to_string()
                .contains("RESOURCE_DETAILS")
        );
        assert_eq!(
            requests[0]["tools"][0]["function"]["name"],
            "read_skill_file"
        );
    }

    /// 多轮流式请求只使用最后一轮的正文作为候选，工具前的说明不能插入文章。
    #[tokio::test]
    async fn final_skill_turn_excludes_intermediate_text() {
        let events = vec![
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::text("我先读取技能"),
            )),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::text("最终正文"),
            )),
            Ok(MultiTurnStreamItem::FinalResponse(
                rig::agent::PromptResponse::new("最终正文", Default::default()),
            )),
        ];
        let (sender, mut receiver) = mpsc::channel(8);
        let result = forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
            .await
            .unwrap();
        assert_eq!(result, "最终正文");
        assert!(receiver.recv().await.unwrap().content.contains("读取技能"));
    }

    /// 搜索失败应展示失败原因，不把工具摘要或失败信息混入候选正文。
    #[tokio::test]
    async fn search_progress_distinguishes_failure_from_success() {
        let events = vec![
            Ok(MultiTurnStreamItem::StreamUserItem(
                StreamedUserContent::ToolResult {
                    tool_result: rig::message::ToolResult {
                        call: rig::message::ToolCallId::new("search-call").unwrap(),
                        provider: None,
                        name: "webSearch".into(),
                        content: vec![rig::message::ToolResultContent::Json {
                            value: serde_json::json!({"ok": false, "error": "搜索服务繁忙"}),
                        }],
                    },
                    internal_call_id: "search-progress".into(),
                },
            )),
            Ok(MultiTurnStreamItem::FinalResponse(
                rig::agent::PromptResponse::new("最终正文", Default::default()),
            )),
        ];
        let (sender, mut receiver) = mpsc::channel(8);
        let result = forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
            .await
            .unwrap();
        assert_eq!(result, "最终正文");
        let event = receiver.recv().await.unwrap();
        assert_eq!(event.kind, "tool");
        assert_eq!(event.id, "search-progress");
        assert_eq!(event.content, "网络搜索失败：搜索服务繁忙");
    }

    /// 已读取网页的标题与 URL 作为来源事件发送，正文不混入工具状态。
    #[tokio::test]
    async fn web_fetch_progress_reports_actual_source() {
        let events = vec![
            Ok(MultiTurnStreamItem::StreamUserItem(
                StreamedUserContent::ToolResult {
                    tool_result: rig::message::ToolResult {
                        call: rig::message::ToolCallId::new("page-call").unwrap(),
                        provider: None,
                        name: "webfetch".into(),
                        content: vec![rig::message::ToolResultContent::Json {
                            value: serde_json::json!({"ok": true, "reference": "P1", "page": {
                                "title": "RAG 教程", "url": "https://example.com/rag", "content": "正文", "truncated": false
                            }}),
                        }],
                    },
                    internal_call_id: "page-progress".into(),
                },
            )),
            Ok(MultiTurnStreamItem::FinalResponse(
                rig::agent::PromptResponse::new("知识点总结", Default::default()),
            )),
        ];
        let (sender, mut receiver) = mpsc::channel(8);
        let result = forward_text_stream(Box::pin(futures_util::stream::iter(events)), sender)
            .await
            .unwrap();
        assert_eq!(result, "知识点总结");
        let event = receiver.recv().await.unwrap();
        assert_eq!(event.content, "网页读取完成：RAG 教程");
        assert_eq!(event.sources[0].reference, "P1");
        assert_eq!(event.sources[0].url, "https://example.com/rag");
    }

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
