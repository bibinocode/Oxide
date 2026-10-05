//! 图像模型适配器：调用兼容 images/generations 协议并限制返回资源大小。

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde::Deserialize;

use crate::{
    entity::{agent_binding, agent_provider},
    infrastructure::secrets,
};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// 绑定优先；未绑定时仅在恰有一个可用图像模型时采用它。
pub async fn image_provider(db: &DatabaseConnection) -> Result<Option<agent_provider::Model>> {
    if let Some(binding) = agent_binding::Entity::find_by_id("image").one(db).await? {
        return Ok(agent_provider::Entity::find_by_id(binding.provider_id)
            .one(db)
            .await?
            .filter(eligible));
    }
    let mut providers = agent_provider::Entity::find()
        .filter(agent_provider::Column::Enabled.eq(true))
        .filter(agent_provider::Column::Capability.eq("image"))
        .filter(agent_provider::Column::EncryptedApiKey.ne(""))
        .limit(2)
        .all(db)
        .await?;
    Ok(if providers.len() == 1 {
        providers.pop().filter(eligible)
    } else {
        None
    })
}

fn eligible(provider: &agent_provider::Model) -> bool {
    provider.enabled
        && provider.capability == "image"
        && matches!(provider.adapter.as_str(), "openai_compatible" | "jimeng")
        && !provider.encrypted_api_key.is_empty()
}

#[derive(Deserialize)]
struct ImageResponse {
    data: Vec<ImageDatum>,
}

#[derive(Deserialize)]
struct ImageDatum {
    b64_json: Option<String>,
    url: Option<String>,
}

/// 密钥只在服务端解密；提供商需暴露 OpenAI/火山方舟兼容生图接口。
pub async fn generate(
    provider: &agent_provider::Model,
    server_key: &str,
    prompt: &str,
) -> Result<Vec<u8>> {
    let key = secrets::open(&provider.encrypted_api_key, server_key, "agent")?;
    let key = String::from_utf8(key).context("图像模型密钥编码无效")?;
    let base = url::Url::parse(&provider.base_url)?;
    if base.scheme() != "https" {
        bail!("图像接口必须使用 HTTPS");
    }
    let endpoint = format!(
        "{}/images/generations",
        provider.base_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let size = if provider.adapter == "jimeng" {
        "2K"
    } else {
        "1536x1024"
    };
    let mut body =
        serde_json::json!({"model": provider.model_id, "prompt": prompt, "size": size, "n": 1});
    // GPT Image 默认返回 base64；其他兼容模型通常需要显式声明。
    if !provider.model_id.starts_with("gpt-image-") && provider.adapter == "openai_compatible" {
        body["response_format"] = serde_json::json!("b64_json");
    }
    let response = client
        .post(endpoint)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("图像模型返回状态 {}", response.status());
    }
    let mut response_bytes = Vec::new();
    let mut response_stream = response.bytes_stream();
    while let Some(part) = response_stream.next().await {
        let part = part?;
        if response_bytes.len() + part.len() > 12 * 1024 * 1024 {
            bail!("图像模型响应过大");
        }
        response_bytes.extend_from_slice(&part);
    }
    let data: ImageResponse =
        serde_json::from_slice(&response_bytes).context("图像模型响应格式无效")?;
    let first = data.data.into_iter().next().context("图像模型未返回图片")?;
    if let Some(encoded) = first.b64_json {
        if encoded.len() > MAX_IMAGE_BYTES * 4 / 3 + 16 {
            bail!("生成图片超过上传上限");
        }
        return STANDARD
            .decode(encoded)
            .context("图像模型返回的图片编码无效");
    }
    let address = first.url.context("图像模型未返回可用图片")?;
    download_public_image(&address).await
}

/// 下载模型签名地址前固定已验证的公共 DNS 结果，拒绝跳转和内网地址。
async fn download_public_image(address: &str) -> Result<Vec<u8>> {
    let url = url::Url::parse(address)?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        bail!("图像模型返回了不安全的地址");
    }
    let host = url.host_str().context("图片地址缺少主机")?.to_owned();
    let mut resolved = tokio::net::lookup_host((host.as_str(), 443)).await?;
    let socket = resolved
        .find(|entry| public_ip(entry.ip()))
        .context("图片地址未解析到公共网络")?;
    if tokio::net::lookup_host((host.as_str(), 443))
        .await?
        .any(|entry| !public_ip(entry.ip()))
    {
        bail!("图片地址包含非公共 IP");
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve(&host, SocketAddr::new(socket.ip(), 443))
        .build()?;
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        bail!("无法下载生成图片");
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_IMAGE_BYTES as u64)
    {
        bail!("生成图片超过上传上限");
    }
    let mut image = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part?;
        if image.len() + part.len() > MAX_IMAGE_BYTES {
            bail!("生成图片超过上传上限");
        }
        image.extend_from_slice(&part);
    }
    Ok(image)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !v.is_private()
                && !v.is_loopback()
                && !v.is_link_local()
                && !v.is_broadcast()
                && !v.is_unspecified()
                && !v.is_documentation()
                && !(v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1]))
                && !(v.octets()[0] == 198 && (18..=19).contains(&v.octets()[1]))
                && !(v.octets()[0] == 192 && v.octets()[1] == 0 && v.octets()[2] == 0)
                && v.octets()[0] < 224
                && v.octets()[0] != 0
        }
        IpAddr::V6(v) => {
            if let Some(mapped) = v.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(mapped));
            }
            !v.is_loopback()
                && !v.is_unique_local()
                && !v.is_unicast_link_local()
                && !v.is_unspecified()
                && !v.is_multicast()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_url_rejects_private_ip_ranges() {
        assert!(!public_ip("127.0.0.1".parse().unwrap()));
        assert!(!public_ip("10.1.2.3".parse().unwrap()));
        assert!(!public_ip("100.64.0.1".parse().unwrap()));
        assert!(!public_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(public_ip("1.1.1.1".parse().unwrap()));
    }
}
