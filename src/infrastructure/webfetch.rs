//! webfetch 公开 HTTPS 内容适配器：逐跳验证、流式限流与多格式内容提取。

use encoding_rs::{Encoding, UTF_8};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{fmt, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use url::{Host, Url};

mod content;

const MAX_RESPONSE_BYTES: usize = 5 * 1_048_576;
const MAX_CONTENT_CHARS: usize = 32_000;
const MAX_REDIRECTS: usize = 3;
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// 模型提交公开页面 URL 与输出格式，HTTP 方法、请求头与跳转预算由服务端控制。
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageInput {
    pub url: String,
    #[serde(default)]
    pub format: FetchFormat,
}

/// 输出格式由模型选取；JSON 格式含页面元信息，原始响应头不向模型开放。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FetchFormat {
    #[default]
    Markdown,
    Text,
    Html,
    Json,
}

/// 只向模型提供经过内容预算处理的页面内容与非敏感元信息。
#[derive(Serialize)]
pub struct PageContent {
    pub title: String,
    pub url: String,
    pub content: String,
    pub format: FetchFormat,
    pub content_type: String,
    pub size_bytes: usize,
    pub truncated: bool,
}

/// 网页读取失败与空内容分离，错误不包含内部地址或上游响应。
#[derive(Debug)]
pub enum PageError {
    InvalidUrl,
    ForbiddenAddress,
    Busy,
    FetchFailed,
    UnsupportedContent,
    TooLarge,
    EmptyContent,
    RedirectLimit,
    BudgetExceeded,
    Disabled,
}

impl fmt::Display for PageError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::InvalidUrl => "仅支持无凭据的公开 HTTPS 页面链接",
            Self::ForbiddenAddress => "目标地址不是可访问的公开 IPv4 地址",
            Self::Busy => "网页读取繁忙，请稍后重试",
            Self::FetchFailed => "网页读取失败，请检查链接或稍后重试",
            Self::UnsupportedContent => "链接未返回可读取的 HTML、文本或 JSON 内容",
            Self::TooLarge => "网页超过读取大小上限",
            Self::EmptyContent => "网页没有可提取的正文",
            Self::RedirectLimit => "网页跳转次数超过上限或跳转地址无效",
            Self::BudgetExceeded => "本次对话的网页读取次数已达上限（2 次）",
            Self::Disabled => "当前任务未授权网页读取",
        })
    }
}

/// 并发预算在请求间共享；每次请求自行绑定经过验证的 DNS 解析结果。
pub struct PageClient {
    concurrency: Arc<Semaphore>,
}

impl PageClient {
    pub fn new() -> Self {
        Self {
            concurrency: Arc::new(Semaphore::new(4)),
        }
    }

    /// 整条跳转链共用并发与时间预算，每一跳重新验证公开地址。
    pub async fn read(&self, input: PageInput) -> Result<PageContent, PageError> {
        let _permit = self
            .concurrency
            .try_acquire()
            .map_err(|_| PageError::Busy)?;
        tokio::time::timeout(Duration::from_secs(30), self.fetch(input))
            .await
            .map_err(|_| PageError::FetchFailed)?
    }

    /// 仅跟随经过同一 URL/DNS 策略重新校验的少量 HTTPS 跳转。
    async fn fetch(&self, input: PageInput) -> Result<PageContent, PageError> {
        let mut url = validate_url(&input.url)?;
        for redirect_count in 0..=MAX_REDIRECTS {
            let response = self.send_to_public_address(&url).await?;
            if response.status().is_redirection() {
                if redirect_count == MAX_REDIRECTS {
                    return Err(PageError::RedirectLimit);
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(PageError::RedirectLimit)?;
                url = redirect_url(&url, location)?;
                continue;
            }
            if !response.status().is_success() {
                return Err(PageError::FetchFailed);
            }
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !supported_content_type(&content_type) {
                return Err(PageError::UnsupportedContent);
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
            {
                return Err(PageError::TooLarge);
            }
            let mut stream = response.bytes_stream();
            let mut body = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| PageError::FetchFailed)?;
                if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err(PageError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            let charset = content_type
                .split(';')
                .find_map(|part| part.trim().strip_prefix("charset="))
                .and_then(|label| Encoding::for_label(label.trim_matches('"').as_bytes()))
                .unwrap_or(UTF_8);
            let (decoded, _, had_errors) = charset.decode(&body);
            if had_errors {
                return Err(PageError::UnsupportedContent);
            }
            let decoded = decoded.into_owned();
            let size_bytes = body.len();
            drop(body);
            return tokio::task::spawn_blocking(move || {
                extract_page(
                    &decoded,
                    url.as_str(),
                    &content_type,
                    size_bytes,
                    input.format,
                )
            })
            .await
            .map_err(|_| PageError::FetchFailed)?;
        }
        Err(PageError::RedirectLimit)
    }

    /// DNS 解析的公网 IPv4 被固定到 TLS 连接，禁用代理与自动重定向。
    async fn send_to_public_address(&self, url: &Url) -> Result<reqwest::Response, PageError> {
        let host = url.host_str().ok_or(PageError::InvalidUrl)?;
        let addresses = tokio::net::lookup_host((host, 443))
            .await
            .map_err(|_| PageError::FetchFailed)?;
        let address = addresses
            .filter_map(|address| match address.ip() {
                std::net::IpAddr::V4(ip) if public_ipv4(ip) => Some(address),
                _ => None,
            })
            .next()
            .ok_or(PageError::ForbiddenAddress)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(BROWSER_USER_AGENT)
            .resolve(host, address)
            .build()
            .map_err(|_| PageError::FetchFailed)?;
        client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| PageError::FetchFailed)
    }
}

/// 文本与 JSON 响应直接输出，HTML 再按格式转换；二进制内容不进入模型。
fn supported_content_type(value: &str) -> bool {
    let mime = value.split(';').next().unwrap_or_default().trim();
    matches!(
        mime,
        "text/html" | "text/plain" | "text/markdown" | "application/json"
    ) || (mime.starts_with("application/") && mime.ends_with("+json"))
}

/// URL 入口拒绝 IP、非标准端口、用户名密码与非公开域名，片段不发送给上游。
fn validate_url(raw: &str) -> Result<Url, PageError> {
    if raw.len() > 2048 {
        return Err(PageError::InvalidUrl);
    }
    let url = Url::parse(raw).map_err(|_| PageError::InvalidUrl)?;
    let Some(Host::Domain(host)) = url.host() else {
        return Err(PageError::InvalidUrl);
    };
    if url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || !host.contains('.')
        || host.ends_with(".local")
        || host.ends_with(".localhost")
        || host.ends_with(".internal")
    {
        return Err(PageError::InvalidUrl);
    }
    let mut url = url;
    url.set_fragment(None);
    Ok(url)
}

/// 相对 Location 以当前页为基准解析，最终仍必须满足公开 HTTPS URL 约束。
fn redirect_url(current: &Url, location: &str) -> Result<Url, PageError> {
    let target = current
        .join(location)
        .map_err(|_| PageError::RedirectLimit)?;
    validate_url(target.as_str()).map_err(|_| PageError::RedirectLimit)
}

/// 只允许可路由公网 IPv4；拒绝私网、环回、链路本地和文档保留段。
fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 0 || b == 168 || (b == 88 && c == 99)))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 192 && b == 0 && c == 2)
        || (a == 203 && b == 0 && c == 113))
}

/// HTML 交由 Readability 识别正文；所有格式共享输出长度和截断标记。
fn extract_page(
    body: &str,
    url: &str,
    content_type: &str,
    size_bytes: usize,
    format: FetchFormat,
) -> Result<PageContent, PageError> {
    let (title, content) = if content_type.starts_with("text/html") {
        content::extract_html(body, url, format)?
    } else {
        (url.to_owned(), body.to_owned())
    };
    let content = content.trim();
    if content.is_empty() {
        return Err(PageError::EmptyContent);
    }
    let truncated = content.chars().count() > MAX_CONTENT_CHARS;
    Ok(PageContent {
        title,
        url: url.into(),
        content: content.chars().take(MAX_CONTENT_CHARS).collect(),
        format,
        content_type: content_type.into(),
        size_bytes,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 跳转目标遵循与入口相同的约束，不能降级为 HTTP 或转入内网地址。
    #[test]
    fn validates_redirects_and_format_contract() {
        let base = validate_url("https://example.com/docs/start").unwrap();
        assert_eq!(
            redirect_url(&base, "../guide#section").unwrap().as_str(),
            "https://example.com/guide"
        );
        for target in [
            "http://example.com/",
            "https://127.0.0.1/",
            "https://host.internal/",
            "https://user:pass@example.com/",
        ] {
            assert!(redirect_url(&base, target).is_err());
        }
        let input: PageInput = serde_json::from_str(r#"{"url":"https://example.com/"}"#).unwrap();
        assert_eq!(input.format, FetchFormat::Markdown);
        assert!(
            serde_json::from_str::<PageInput>(r#"{"url":"https://example.com/","format":"pdf"}"#)
                .is_err()
        );
        assert!(supported_content_type("application/ld+json; charset=utf-8"));
        assert!(!supported_content_type("image/png; name=+json"));
    }

    /// 非 HTML 内容直接返回，截断以 Unicode 字符计数且显式标记。
    #[test]
    fn handles_plain_responses_and_unicode_truncation() {
        let json = r#"{"知识点":"RAG"}"#;
        let page = extract_page(
            json,
            "https://example.com/data",
            "application/json",
            json.len(),
            FetchFormat::Json,
        )
        .unwrap();
        assert_eq!(page.content, json);
        assert_eq!(page.size_bytes, json.len());
        let long = "文".repeat(MAX_CONTENT_CHARS + 1);
        let page = extract_page(
            &long,
            "https://example.com/",
            "text/plain",
            long.len(),
            FetchFormat::Text,
        )
        .unwrap();
        assert!(page.truncated);
        assert_eq!(page.content.chars().count(), MAX_CONTENT_CHARS);
    }

    #[test]
    fn rejects_private_targets_and_extracts_article() {
        for url in [
            "http://example.com/",
            "https://127.0.0.1/",
            "https://localhost/",
            "https://user:pass@example.com/",
            "https://example.com:8443/",
        ] {
            assert!(validate_url(url).is_err());
        }
        assert!(!public_ipv4(Ipv4Addr::new(169, 254, 1, 1)));
        assert!(!public_ipv4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(public_ipv4(Ipv4Addr::new(1, 1, 1, 1)));
        let page = extract_page(
            "<title>标题</title><nav><p>导航</p></nav><main><article><h1>知识点</h1><p>RAG 使用检索增强生成。</p></article></main>",
            "https://example.com/article",
            "text/html",
            0,
            FetchFormat::Markdown,
        )
        .unwrap();
        assert!(page.content.contains("RAG 使用检索增强生成"));
        assert!(!page.content.contains("导航"));
    }

    #[tokio::test]
    #[ignore = "需要公网访问，用于手动验收目标站点的正文提取"]
    async fn reads_public_article() {
        let page = PageClient::new()
            .read(PageInput {
                url: "https://javabetter.cn/sidebar/itwanger/paicli/build-agent-p4-rag.html".into(),
                format: FetchFormat::Markdown,
            })
            .await
            .unwrap();
        assert!(page.content.contains("RAG"));
        assert!(page.content.chars().count() > 1000);
        assert!(!page.content.contains("登录 注册"));
    }
}
