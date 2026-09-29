//! 阿里云 OSS 与七牛 Kodo 的 S3 兼容对象存储适配器。

use std::{env, io::Cursor, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use axum::body::Bytes;
use object_store::{ObjectStore, ObjectStoreExt, aws::AmazonS3Builder, path::Path};
use qiniu_sdk::{
    download::{DownloadManager, StaticDomainsUrlsGenerator, UrlsSigner},
    objects::{ObjectsManager, apis::credential::Credential},
    upload::{
        ObjectParams, SinglePartUploader, UploadManager, UploadTokenSigner,
        apis::http_client::mime::Mime,
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    entity::{status::StorageProviderKind, storage_provider},
    infrastructure::secrets,
};

/// 根据数据库中稳定的提供商 ID 读取对应环境变量。
fn setting(id: &str, suffix: &str) -> Result<String> {
    let name = format!("STORAGE_{}_{}", id.to_ascii_uppercase(), suffix);
    env::var(&name).with_context(|| format!("缺少存储配置 {name}"))
}

/// 对象存储访问密钥，仅存在于服务端内存和加密后的数据库字段。
#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
}

/// 加密凭据；每次保存生成独立随机 nonce。
pub fn encrypt_credentials(value: &Credentials, server_key: &str) -> Result<String> {
    secrets::seal(&serde_json::to_vec(value)?, server_key, "storage")
}

/// 解密凭据；认证失败时不输出密文或明文。
fn decrypt_credentials(value: &str, server_key: &str) -> Result<Credentials> {
    Ok(serde_json::from_slice(&secrets::open(
        value, server_key, "storage",
    )?)?)
}

/// 从加密配置或旧环境变量读取密钥，不向 API 和日志暴露内容。
fn credentials(provider: &storage_provider::Model, server_key: &str) -> Result<Credentials> {
    match &provider.encrypted_credentials {
        Some(value) => decrypt_credentials(value, server_key),
        None => Ok(Credentials {
            access_key: setting(&provider.id, "ACCESS_KEY")?,
            secret_key: setting(&provider.id, "SECRET_KEY")?,
        }),
    }
}

/// 七牛原生接口只需要空间名称；S3 端点不参与上传。
fn qiniu_bucket(provider: &storage_provider::Model) -> Result<String> {
    provider
        .bucket
        .clone()
        .map(Ok)
        .unwrap_or_else(|| setting(&provider.id, "BUCKET"))
}

/// 在保存或启用提供商前检查必要配置。
pub fn configured(provider: &storage_provider::Model, server_key: &str) -> Result<()> {
    if provider.kind == StorageProviderKind::QiniuKodo {
        qiniu_bucket(provider)?;
        credentials(provider, server_key)?;
    } else {
        client(provider, server_key)?;
    }
    Ok(())
}

/// 为阿里云 OSS 构建 S3 兼容客户端。
pub fn client(
    provider: &storage_provider::Model,
    server_key: &str,
) -> Result<Arc<dyn ObjectStore>> {
    if !provider
        .id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        bail!("存储提供商 ID 格式无效");
    }
    let endpoint = provider
        .endpoint
        .clone()
        .map(Ok)
        .unwrap_or_else(|| setting(&provider.id, "ENDPOINT"))?;
    let bucket = provider
        .bucket
        .clone()
        .map(Ok)
        .unwrap_or_else(|| setting(&provider.id, "BUCKET"))?;
    let region = provider
        .region
        .clone()
        .map(Ok)
        .unwrap_or_else(|| setting(&provider.id, "REGION"))?;
    let credentials = credentials(provider, server_key)?;
    if !endpoint.starts_with("https://") {
        bail!("对象存储端点必须使用 HTTPS");
    }
    let store = AmazonS3Builder::new()
        .with_endpoint(endpoint)
        .with_bucket_name(bucket)
        .with_region(region)
        .with_access_key_id(credentials.access_key)
        .with_secret_access_key(credentials.secret_key)
        .with_virtual_hosted_style_request(true)
        .build()
        .context("初始化对象存储客户端失败")?;
    Ok(Arc::new(store))
}

/// 将图片写入其所属的 OSS；七牛使用官方上传凭证协议。
pub async fn put(
    provider: &storage_provider::Model,
    server_key: &str,
    key: &str,
    bytes: Bytes,
    mime_type: &str,
) -> Result<()> {
    if provider.kind == StorageProviderKind::AliyunOss {
        client(provider, server_key)?
            .put(&Path::from(key), bytes.into())
            .await?;
        return Ok(());
    }
    let credentials = credentials(provider, server_key)?;
    let bucket = qiniu_bucket(provider)?;
    let key = key.to_owned();
    let mime_type: Mime = mime_type.parse().context("图片 MIME 类型无效")?;
    tokio::task::spawn_blocking(move || {
        let manager = UploadManager::new(UploadTokenSigner::new_credential_provider(
            Credential::new(credentials.access_key, credentials.secret_key),
            bucket,
            Duration::from_secs(3600),
        ));
        manager.form_uploader().upload_reader(
            Cursor::new(bytes),
            ObjectParams::builder()
                .object_name(key.as_str())
                .content_type(mime_type)
                .build(),
        )?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

/// 读取图片内容；七牛下载 URL 由官方 SDK 签名，兼容私有空间。
pub async fn get(provider: &storage_provider::Model, server_key: &str, key: &str) -> Result<Bytes> {
    if provider.kind == StorageProviderKind::AliyunOss {
        return Ok(client(provider, server_key)?
            .get(&Path::from(key))
            .await?
            .bytes()
            .await?);
    }
    let credentials = credentials(provider, server_key)?;
    let domain = url::Url::parse(&provider.public_base_url)?
        .host_str()
        .context("七牛公开域名缺失")?
        .to_owned();
    let key = key.to_owned();
    let data = tokio::task::spawn_blocking(move || {
        let manager = DownloadManager::new(UrlsSigner::new(
            Credential::new(credentials.access_key, credentials.secret_key),
            StaticDomainsUrlsGenerator::new(domain),
        ));
        let mut data = Vec::new();
        manager.download(&key)?.to_writer(&mut data)?;
        Ok::<_, anyhow::Error>(data)
    })
    .await??;
    Ok(Bytes::from(data))
}

/// 删除对象；数据库记录由调用方在对象删除成功后处理。
pub async fn delete(provider: &storage_provider::Model, server_key: &str, key: &str) -> Result<()> {
    if provider.kind == StorageProviderKind::AliyunOss {
        client(provider, server_key)?
            .delete(&Path::from(key))
            .await?;
        return Ok(());
    }
    let credentials = credentials(provider, server_key)?;
    let bucket = qiniu_bucket(provider)?;
    let key = key.to_owned();
    tokio::task::spawn_blocking(move || {
        let manager = ObjectsManager::new(Credential::new(
            credentials.access_key,
            credentials.secret_key,
        ));
        manager.bucket(bucket).delete_object(&key).call()?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_round_trip_and_reject_wrong_key() {
        let value = Credentials {
            access_key: "access".into(),
            secret_key: "secret".into(),
        };
        let encrypted = encrypt_credentials(&value, "server-secret").unwrap();
        assert!(!encrypted.contains("secret"));
        assert_eq!(
            decrypt_credentials(&encrypted, "server-secret")
                .unwrap()
                .secret_key,
            "secret"
        );
        assert!(decrypt_credentials(&encrypted, "another-secret").is_err());
    }

    #[test]
    fn qiniu_configuration_does_not_require_s3_fields() {
        let credentials = Credentials {
            access_key: "access".into(),
            secret_key: "secret".into(),
        };
        let provider = storage_provider::Model {
            id: "TEST_QINIU".into(),
            kind: StorageProviderKind::QiniuKodo,
            name: "七牛测试".into(),
            public_base_url: "https://cdn.example.com".into(),
            endpoint: None,
            bucket: Some("example-bucket".into()),
            region: None,
            encrypted_credentials: Some(
                encrypt_credentials(&credentials, "server-secret").unwrap(),
            ),
            upload_enabled: true,
            created_at: chrono::Utc::now(),
        };
        assert!(configured(&provider, "server-secret").is_ok());
        assert!(configured(&provider, "wrong-secret").is_err());
    }
}
