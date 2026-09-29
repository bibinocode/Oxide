//! 阿里云 OSS 与七牛 Kodo 的 S3 兼容对象存储适配器。

use std::{env, sync::Arc};

use anyhow::{Context, Result, bail};
use object_store::{ObjectStore, aws::AmazonS3Builder};
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

/// 为阿里云 OSS 或七牛 Kodo 构建 S3 兼容客户端。
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
    let credentials = match &provider.encrypted_credentials {
        Some(value) => decrypt_credentials(value, server_key)?,
        None => Credentials {
            access_key: setting(&provider.id, "ACCESS_KEY")?,
            secret_key: setting(&provider.id, "SECRET_KEY")?,
        },
    };
    if !endpoint.starts_with("https://") {
        bail!("对象存储端点必须使用 HTTPS");
    }
    let virtual_host = provider.kind == StorageProviderKind::AliyunOss;
    let store = AmazonS3Builder::new()
        .with_endpoint(endpoint)
        .with_bucket_name(bucket)
        .with_region(region)
        .with_access_key_id(credentials.access_key)
        .with_secret_access_key(credentials.secret_key)
        .with_virtual_hosted_style_request(virtual_host)
        .build()
        .context("初始化对象存储客户端失败")?;
    Ok(Arc::new(store))
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
    fn qiniu_client_uses_database_configuration() {
        let credentials = Credentials {
            access_key: "access".into(),
            secret_key: "secret".into(),
        };
        let provider = storage_provider::Model {
            id: "TEST_QINIU".into(),
            kind: StorageProviderKind::QiniuKodo,
            name: "七牛测试".into(),
            public_base_url: "https://cdn.example.com".into(),
            endpoint: Some("https://s3-cn-east-1.qiniucs.com".into()),
            bucket: Some("example-bucket".into()),
            region: Some("cn-east-1".into()),
            encrypted_credentials: Some(
                encrypt_credentials(&credentials, "server-secret").unwrap(),
            ),
            upload_enabled: true,
            created_at: chrono::Utc::now(),
        };
        assert!(client(&provider, "server-secret").is_ok());
        assert!(client(&provider, "wrong-secret").is_err());
    }
}
