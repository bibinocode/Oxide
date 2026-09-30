//! 智谱网络搜索适配器：固定 HTTPS 端点、私有环境密钥、超时与有界响应。

use crate::domain::agent_tool::{SearchEngine, SearchRecency, WebSearchSettings, normalize_domain};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use utoipa::ToSchema;

/// 首个工具的模型与管理员测试输入，不允许模型指定引擎或增加返回条数。
#[derive(Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub recency: Option<SearchRecency>,
    #[serde(default)]
    pub domain: Option<String>,
}

/// 经过 URL 与字符预算净化的搜索来源，不直接传递任意上游 JSON。
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SearchHit {
    pub title: String,
    pub content: String,
    pub url: String,
    pub source: String,
    pub reference: String,
    pub publish_date: Option<String>,
    /// 表示上下文预算截断，不能声称已经读取完整网页。
    pub truncated: bool,
}

/// 空结果是有效响应，失败与空结果明确分离。
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    pub query: String,
    pub request_id: String,
    pub results: Vec<SearchHit>,
}

/// 面向编辑器的来源索引，不携带搜索摘要或上游任意字段。
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SearchSource {
    pub reference: String,
    pub title: String,
    pub url: String,
    pub source: String,
    pub publish_date: Option<String>,
}

/// 可直接展示的错误不会包含 API Key、原始上游响应或内部路径。
#[derive(Debug)]
pub enum SearchError {
    InvalidInput(String),
    NotConfigured,
    Disabled,
    Busy,
    Timeout,
    Authentication,
    Upstream,
    InvalidResponse,
    BudgetExceeded,
    Configuration,
}
impl fmt::Display for SearchError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::InvalidInput(message) => message,
            Self::NotConfigured => "未配置 WEB_SEARCH_API_KEY，请配置后重启后端",
            Self::Disabled => "webSearch 已停用或未授权当前任务",
            Self::Busy => "搜索服务繁忙，请稍后重试",
            Self::Timeout => "网络搜索超时，请稍后重试",
            Self::Authentication => "网络搜索认证失败，请检查服务端密钥",
            Self::Upstream => "网络搜索服务返回错误，请检查服务状态、额度或稍后重试",
            Self::InvalidResponse => "网络搜索返回格式无效或超过响应预算",
            Self::BudgetExceeded => "本次对话的搜索次数已达上限（3 次）",
            Self::Configuration => "工具配置暂时不可用，请检查服务端配置文件",
        })
    }
}
impl std::error::Error for SearchError {}

/// 复用连接池与全局并发预算；密钥不会实现 Debug 或 Serialize。
pub struct SearchClient {
    client: reqwest::Client,
    key: Option<String>,
    endpoint: String,
    concurrency: Arc<Semaphore>,
}

impl SearchClient {
    /// 仅测试可替换端点，生产始终使用固定的智谱 HTTPS 地址。
    #[cfg(test)]
    pub(crate) fn mock(endpoint: String) -> Self {
        let mut client = Self::new(Some("test-search-secret".into())).unwrap();
        client.endpoint = endpoint;
        client
    }
    /// 端点固定，管理员与模型都不能将 Bearer 密钥发送到自定义 URL。
    pub fn new(key: Option<String>) -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(25))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            key: key
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            endpoint: "https://open.bigmodel.cn/api/paas/v4/web_search".into(),
            concurrency: Arc::new(Semaphore::new(4)),
        })
    }
    /// 仅暴露配置是否存在，不返回密钥或截取后的密钥。
    pub fn configured(&self) -> bool {
        self.key.is_some()
    }

    /// 单次检索不自动重试，防止失败重试产生额外计费。
    pub async fn search(
        &self,
        settings: &WebSearchSettings,
        input: SearchInput,
    ) -> Result<SearchResponse, SearchError> {
        if !settings.enabled {
            return Err(SearchError::Disabled);
        }
        let key = self.key.as_ref().ok_or(SearchError::NotConfigured)?;
        let query = input.query.trim();
        if query.is_empty() || query.chars().count() > 70 {
            return Err(SearchError::InvalidInput(
                "搜索关键词须为 1–70 字符，请使用简短关键词".into(),
            ));
        }
        let domain = normalize_domain(input.domain.as_deref())
            .map_err(|_| SearchError::InvalidInput("搜索域名格式无效，请使用裸 DNS 域名".into()))?;
        if settings.search_domain_filter.is_some()
            && domain.is_some()
            && settings.search_domain_filter != domain
        {
            return Err(SearchError::InvalidInput(
                "不能覆盖管理员指定的搜索域名".into(),
            ));
        }
        let domain = settings.search_domain_filter.clone().or(domain);
        if settings.search_engine == SearchEngine::Quark && domain.is_some() {
            return Err(SearchError::InvalidInput("夸克引擎不支持域名筛选".into()));
        }
        let _permit = self
            .concurrency
            .clone()
            .try_acquire_owned()
            .map_err(|_| SearchError::Busy)?;
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut payload = serde_json::json!({"search_query": query, "search_engine": settings.search_engine, "search_intent": false,
            "search_recency_filter": input.recency.unwrap_or(settings.search_recency_filter), "content_size": settings.content_size, "request_id": request_id});
        if settings.search_engine != SearchEngine::Quark {
            payload["count"] = serde_json::json!(settings.count);
        }
        if let Some(domain) = domain {
            payload["search_domain_filter"] = serde_json::json!(domain);
        }
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .map_err(|cause| {
                if cause.is_timeout() {
                    SearchError::Timeout
                } else {
                    SearchError::Upstream
                }
            })?;
        match response.status().as_u16() {
            401 | 403 => return Err(SearchError::Authentication),
            429 => return Err(SearchError::Busy),
            status if !(200..300).contains(&status) => return Err(SearchError::Upstream),
            _ => {}
        }
        if response
            .content_length()
            .is_some_and(|length| length > 2_097_152)
        {
            return Err(SearchError::InvalidResponse);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|cause| {
                if cause.is_timeout() {
                    SearchError::Timeout
                } else {
                    SearchError::Upstream
                }
            })?;
            if bytes.len() + chunk.len() > 2_097_152 {
                return Err(SearchError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| SearchError::InvalidResponse)?;
        if body.get("error").is_some_and(|value| !value.is_null()) {
            return Err(SearchError::Upstream);
        }
        let rows = body["search_result"]
            .as_array()
            .ok_or(SearchError::InvalidResponse)?;
        let mut results = Vec::new();
        for row in rows {
            if results.len() >= settings.count as usize {
                break;
            }
            let Some(link) = row["link"].as_str().filter(|value| value.len() <= 2048) else {
                continue;
            };
            let Ok(url) = url::Url::parse(link) else {
                continue;
            };
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                continue;
            }
            let raw = row["content"].as_str().unwrap_or_default();
            results.push(SearchHit {
                title: clip(row["title"].as_str().unwrap_or("未命名来源"), 200),
                content: clip(raw, 1600),
                url: url.to_string(),
                source: clip(row["media"].as_str().unwrap_or_default(), 100),
                reference: format!("S{}", results.len() + 1),
                publish_date: row["publish_date"].as_str().map(|date| clip(date, 40)),
                truncated: raw.chars().count() > 1600,
            });
        }
        if !rows.is_empty() && results.is_empty() {
            return Err(SearchError::InvalidResponse);
        }
        Ok(SearchResponse {
            query: query.into(),
            request_id,
            results,
        })
    }
}

/// 裁剪 Unicode 字符并去除非排版控制字符，不拆断 UTF-8 字节。
fn clip(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 认证错误和恶意响应内容不得泄漏；空结果、错误与超预算分别处理。
    #[tokio::test]
    async fn errors_are_safe_and_empty_results_are_successful() {
        use axum::{
            Json, Router,
            http::StatusCode,
            response::{IntoResponse, Response},
            routing::post,
        };
        let app = Router::new().route(
            "/search",
            post(|Json(input): Json<serde_json::Value>| async move {
                let response: Response = match input["search_query"].as_str().unwrap() {
                    "auth" => (
                        StatusCode::UNAUTHORIZED,
                        "test-search-secret private upstream details",
                    )
                        .into_response(),
                    "busy" => StatusCode::TOO_MANY_REQUESTS.into_response(),
                    "error" => {
                        Json(serde_json::json!({"error": {"message": "test-search-secret"}}))
                            .into_response()
                    }
                    "invalid" => Json(
                        serde_json::json!({"search_result": [{"link": "javascript:alert(1)"}]}),
                    )
                    .into_response(),
                    "large" => "x".repeat(2_097_153).into_response(),
                    _ => Json(serde_json::json!({"search_result": []})).into_response(),
                };
                response
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = SearchClient::mock(format!("http://{address}/search"));
        for (query, expected) in [
            ("auth", "认证失败"),
            ("busy", "繁忙"),
            ("error", "服务返回错误"),
            ("invalid", "格式无效"),
            ("large", "响应预算"),
        ] {
            let error = client
                .search(
                    &WebSearchSettings::default(),
                    SearchInput {
                        query: query.into(),
                        recency: None,
                        domain: None,
                    },
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains(expected));
            assert!(!error.to_string().contains("test-search-secret"));
        }
        let empty = client
            .search(
                &WebSearchSettings::default(),
                SearchInput {
                    query: "empty".into(),
                    recency: None,
                    domain: None,
                },
            )
            .await
            .unwrap();
        assert!(empty.results.is_empty());
        server.abort();
    }
    /// 本地模拟官方协议，验证请求字段、密钥、结果净化及错误，不访问实际服务。
    #[tokio::test]
    async fn validates_protocol_and_bounds_untrusted_results() {
        use axum::{Json, Router, http::HeaderMap, routing::post};
        let app = Router::new().route("/search", post(|headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
            assert_eq!(headers["authorization"], "Bearer test-key"); assert_eq!(body["search_engine"], "search_std"); assert_eq!(body["search_intent"], false); assert_eq!(body["count"], 5);
            Json(serde_json::json!({"search_result": [{"title": "Rust 文档", "content": "字".repeat(2000), "link": "https://www.rust-lang.org/", "publish_date": "2026-09-30"}, {"title": "unsafe", "link": "javascript:alert(1)"}]}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut client = SearchClient::new(Some("test-key".into())).unwrap();
        client.endpoint = format!("http://{address}/search");
        let result = client
            .search(
                &WebSearchSettings::default(),
                SearchInput {
                    query: "Rust 文档".into(),
                    recency: None,
                    domain: None,
                },
            )
            .await
            .unwrap();
        server.abort();
        assert_eq!(result.results.len(), 1);
        assert!(result.results[0].truncated);
        assert_eq!(result.results[0].content.chars().count(), 1600);
        assert!(matches!(
            client
                .search(
                    &WebSearchSettings::default(),
                    SearchInput {
                        query: "字".repeat(71),
                        recency: None,
                        domain: None
                    }
                )
                .await,
            Err(SearchError::InvalidInput(_))
        ));
    }
}
