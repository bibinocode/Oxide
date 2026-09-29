//! 将 Notion 的临时文件型图片转存到当前对象存储。

use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, bail};
use axum::body::Bytes;
use chrono::Utc;
use reqwest::Client;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

use crate::{
    domain::notion::Block,
    entity::{asset, status::AssetVisibility, storage_provider},
    infrastructure::storage,
};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMAGES: usize = 30;

/// 返回需要转存的块 ID 与 Notion 签名下载链接。
fn file_images(blocks: &[Block], output: &mut Vec<(Uuid, String)>) {
    for block in blocks {
        if block.kind == "image"
            && block.data["type"] == "file"
            && let Some(url) = block
                .data
                .pointer("/file/url")
                .and_then(serde_json::Value::as_str)
        {
            output.push((block.id, url.to_owned()));
        }
        file_images(&block.children, output);
    }
}

/// 检查页面是否含有会过期的 Notion 图片。
pub fn needs_storage(blocks: &[Block]) -> bool {
    let mut images = Vec::new();
    file_images(blocks, &mut images);
    !images.is_empty()
}

/// Notion 签名文件 URL 仅能指向其官方托管域名，且不跟随跳转。
fn allowed_file_url(input: &str) -> bool {
    url::Url::parse(input).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some_and(|host| {
                [
                    "amazonaws.com",
                    "notion.so",
                    "notion-static.com",
                    "notionusercontent.com",
                ]
                .iter()
                .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
            })
    })
}

/// 下载、验证图片并写入素材库；返回块 ID 到稳定媒体地址的映射。
pub async fn import_images(
    db: &sea_orm::DatabaseConnection,
    provider: &storage_provider::Model,
    server_key: &str,
    blocks: &[Block],
) -> Result<(HashMap<Uuid, String>, Vec<asset::Model>)> {
    let mut images = Vec::new();
    file_images(blocks, &mut images);
    if images.len() > MAX_IMAGES {
        bail!("单篇 Notion 文章最多导入 {MAX_IMAGES} 张图片");
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut urls = HashMap::new();
    let mut created = Vec::new();
    for (block_id, url) in images {
        let result = import_one(db, provider, server_key, &client, &url).await;
        match result {
            Ok(row) => {
                urls.insert(block_id, format!("/media/{}", row.public_id));
                created.push(row);
            }
            Err(error) => {
                cleanup(db, provider, server_key, &created).await;
                return Err(error);
            }
        }
    }
    Ok((urls, created))
}

async fn import_one(
    db: &sea_orm::DatabaseConnection,
    provider: &storage_provider::Model,
    server_key: &str,
    client: &Client,
    url: &str,
) -> Result<asset::Model> {
    if !allowed_file_url(url) {
        bail!("Notion 图片下载域名无效");
    }
    let mut response = client
        .get(url)
        .send()
        .await
        .context("下载 Notion 图片失败")?;
    if !response.status().is_success() {
        bail!("Notion 图片下载返回 HTTP {}", response.status().as_u16());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE_BYTES as u64)
    {
        bail!("Notion 图片超过 8 MiB");
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if data.len() + chunk.len() > MAX_IMAGE_BYTES {
            bail!("Notion 图片超过 8 MiB");
        }
        data.extend_from_slice(&chunk);
    }
    let kind = infer::get(&data).context("Notion 图片格式无效")?;
    let (mime, extension) = match kind.mime_type() {
        "image/png" => ("image/png", "png"),
        "image/jpeg" => ("image/jpeg", "jpg"),
        "image/gif" => ("image/gif", "gif"),
        "image/webp" => ("image/webp", "webp"),
        _ => bail!("Notion 图片仅支持 PNG、JPEG、GIF 和 WebP"),
    };
    let dimensions = imagesize::blob_size(&data).context("Notion 图片尺寸无效")?;
    if dimensions.width > 10_000 || dimensions.height > 10_000 {
        bail!("Notion 图片尺寸过大");
    }
    let public_id = Uuid::new_v4();
    let key = format!("assets/{public_id}.{extension}");
    let size_bytes = data.len() as i64;
    storage::put(provider, server_key, &key, Bytes::from(data), mime).await?;
    let row = asset::ActiveModel {
        public_id: Set(public_id),
        provider_id: Set(provider.id.clone()),
        object_key: Set(key.clone()),
        mime_type: Set(mime.into()),
        size_bytes: Set(size_bytes),
        width: Set(Some(dimensions.width as i32)),
        height: Set(Some(dimensions.height as i32)),
        visibility: Set(AssetVisibility::Public),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await;
    match row {
        Ok(row) => Ok(row),
        Err(error) => {
            let _ = storage::delete(provider, server_key, &key).await;
            Err(error.into())
        }
    }
}

/// 文章事务失败时尽力回收本次导入的图片。
pub async fn cleanup(
    db: &sea_orm::DatabaseConnection,
    provider: &storage_provider::Model,
    server_key: &str,
    rows: &[asset::Model],
) {
    for row in rows {
        if let Err(error) = storage::delete(provider, server_key, &row.object_key).await {
            tracing::warn!(error = %error, "回收 Notion 图片对象失败");
            continue;
        }
        if let Err(error) = asset::Entity::delete_by_id(row.id).exec(db).await {
            tracing::warn!(error = %error, "回收 Notion 图片记录失败");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_notion_file_hosts() {
        assert!(allowed_file_url(
            "https://prod-files-secure.s3.us-west-2.amazonaws.com/a"
        ));
        assert!(!allowed_file_url(
            "http://prod-files-secure.s3.us-west-2.amazonaws.com/a"
        ));
        assert!(!allowed_file_url("https://amazonaws.com.evil.test/a"));
    }
}
