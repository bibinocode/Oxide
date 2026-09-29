//! 外链元信息抓取；DNS 校验、重定向禁用和响应限额防止内网探测。
use std::{
    net::{IpAddr, SocketAddr},
    sync::OnceLock,
    time::Duration,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use utoipa::{IntoParams, ToSchema};

use super::super::{ApiError, AppState, error};

/// 公开预览请求限制同时抓取数，避免耗尽 HTTP 连接和内存。
static FETCH_LIMIT: OnceLock<Semaphore> = OnceLock::new();

/// 需要预览的完整外链。
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PreviewQuery {
    pub url: String,
}

/// 链接悬停卡使用的安全元数据。
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct LinkPreview {
    pub title: String,
    pub description: Option<String>,
    pub domain: String,
    pub icon_url: Option<String>,
    pub image_url: Option<String>,
}

/// 只接受公网 IP；拒绝回环、私网、链路本地、运营商共享地址及特殊网络。
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 0 || b == 168))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 2)
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            (segments[0] & 0xe000) == 0x2000
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] == 0 || segments[1] == 0x0db8))
                && v6.to_ipv4_mapped().is_none()
        }
    }
}

/// 选择 meta 属性，避免用正则直接解析 HTML。
fn meta(document: &Html, key: &str) -> Option<String> {
    let selector = Selector::parse("meta[property], meta[name]").ok()?;
    document.select(&selector).find_map(|element| {
        let attrs = element.value();
        (attrs.attr("property") == Some(key) || attrs.attr("name") == Some(key))
            .then(|| attrs.attr("content").unwrap_or("").trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// 相对资源地址以目标网页为基准，禁止非 HTTP(S) 资源。
fn resource_url(base: &url::Url, value: &str) -> Option<String> {
    let parsed = base.join(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// 在已解析且通过 DNS 检查的域名上取有限大小的 HTML。
async fn fetch(url: &url::Url) -> anyhow::Result<LinkPreview> {
    let host = url.host_str().ok_or_else(|| anyhow::anyhow!("域名缺失"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("端口无效"))?;
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await?.collect();
    if addresses.is_empty() || addresses.iter().any(|addr| !public_ip(addr.ip())) {
        anyhow::bail!("目标地址不是公网");
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addresses)
        .user_agent("OxideLinkPreview/1.0")
        .build()?;
    let mut response = client.get(url.as_str()).send().await?;
    if !response.status().is_success()
        || !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/html"))
    {
        anyhow::bail!("目标不是 HTML 页面");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > 256 * 1024 {
            anyhow::bail!("网页元数据过大");
        }
        body.extend_from_slice(&chunk);
    }
    let document = Html::parse_document(&String::from_utf8_lossy(&body));
    let title_selector = Selector::parse("title").expect("固定选择器有效");
    let icon_selector = Selector::parse("link[rel~='icon']").expect("固定选择器有效");
    let title = meta(&document, "og:title")
        .or_else(|| {
            document
                .select(&title_selector)
                .next()
                .map(|node| node.text().collect::<String>().trim().to_string())
        })
        .unwrap_or_else(|| host.to_string());
    let icon_url = document
        .select(&icon_selector)
        .next()
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| resource_url(url, href))
        .or_else(|| resource_url(url, "/favicon.ico"));
    Ok(LinkPreview {
        title: title.chars().take(160).collect(),
        description: meta(&document, "og:description")
            .or_else(|| meta(&document, "description"))
            .map(|value| value.chars().take(300).collect()),
        domain: host.to_string(),
        icon_url,
        image_url: meta(&document, "og:image").and_then(|value| resource_url(url, &value)),
    })
}

/// 按需读取外链预览；Redis 缓存避免访客重复访问目标站。
#[utoipa::path(get, path = "/api/v1/link-preview", params(PreviewQuery), responses((status = 200, body = LinkPreview), (status = 400, body = ApiError)), tag = "articles")]
pub async fn preview(State(state): State<AppState>, Query(query): Query<PreviewQuery>) -> Response {
    if query.url.len() > 2048 {
        return error(StatusCode::BAD_REQUEST, "invalid_url", "链接过长");
    }
    let Ok(url) = url::Url::parse(&query.url) else {
        return error(StatusCode::BAD_REQUEST, "invalid_url", "链接格式无效");
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.username() != ""
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 80 && port != 443)
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_url",
            "仅支持公开 HTTP(S) 网页",
        );
    }
    let cache_key = format!(
        "link-preview:v1:{}",
        hex::encode(Sha256::digest(url.as_str().as_bytes()))
    );
    if let Ok(mut connection) = state.redis.get_multiplexed_async_connection().await {
        let cached: redis::RedisResult<Option<String>> = redis::cmd("GET")
            .arg(&cache_key)
            .query_async(&mut connection)
            .await;
        if let Ok(Some(cached)) = cached
            && let Ok(value) = serde_json::from_str::<LinkPreview>(&cached)
        {
            return Json(value).into_response();
        }
    }
    let Ok(_permit) = FETCH_LIMIT.get_or_init(|| Semaphore::new(4)).try_acquire() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "preview_busy",
            "预览服务繁忙",
        );
    };
    let value = match fetch(&url).await {
        Ok(value) => value,
        Err(err) => {
            tracing::debug!(error = %err, "抓取外链预览失败");
            return error(
                StatusCode::BAD_GATEWAY,
                "preview_unavailable",
                "暂时无法预览此网页",
            );
        }
    };
    if let Ok(mut connection) = state.redis.get_multiplexed_async_connection().await
        && let Ok(json) = serde_json::to_string(&value)
    {
        let _: redis::RedisResult<()> = redis::cmd("SETEX")
            .arg(&cache_key)
            .arg(86400)
            .arg(json)
            .query_async(&mut connection)
            .await;
    }
    Json(value).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};
    #[test]
    fn rejects_internal_networks() {
        for ip in [
            Ipv4Addr::LOCALHOST.into(),
            Ipv4Addr::new(10, 0, 0, 1).into(),
            Ipv4Addr::new(169, 254, 169, 254).into(),
            Ipv6Addr::LOCALHOST.into(),
        ] {
            assert!(!public_ip(ip));
        }
        assert!(public_ip(Ipv4Addr::new(8, 8, 8, 8).into()));
    }
}
