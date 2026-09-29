//! 使用七牛官方 SDK 对后台保存的 Kodo 空间执行只读连接检查。

use std::env;

use anyhow::{Context, Result};
use axum::body::Bytes;
use qiniu_sdk::objects::{ObjectsManager, apis::credential::Credential};
use rust_oxide::{
    config::Config,
    entity::{status::StorageProviderKind, storage_provider},
    infrastructure::{
        secrets,
        storage::{self, Credentials},
    },
};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

/// 读取数据库配置；仅在旧记录没有加密凭据时回退到环境变量。
fn credentials(provider: &storage_provider::Model, server_key: &str) -> Result<Credentials> {
    if let Some(encrypted) = &provider.encrypted_credentials {
        return serde_json::from_slice(&secrets::open(encrypted, server_key, "storage")?)
            .context("解码七牛凭据失败");
    }
    let prefix = format!("STORAGE_{}", provider.id.to_ascii_uppercase());
    Ok(Credentials {
        access_key: env::var(format!("{prefix}_ACCESS_KEY")).context("缺少七牛 Access Key")?,
        secret_key: env::var(format!("{prefix}_SECRET_KEY")).context("缺少七牛 Secret Key")?,
    })
}

/// 获取空间名称，兼容已有的环境变量配置。
fn bucket_name(provider: &storage_provider::Model) -> Result<String> {
    provider.bucket.clone().map(Ok).unwrap_or_else(|| {
        env::var(format!(
            "STORAGE_{}_BUCKET",
            provider.id.to_ascii_uppercase()
        ))
        .context("缺少七牛空间名称")
    })
}

/// 每个已配置的七牛空间只请求一页、至多一条；不输出返回的对象信息。
fn check_provider(provider: &storage_provider::Model, server_key: &str) -> Result<()> {
    let credentials = credentials(provider, server_key)?;
    let bucket_name = bucket_name(provider)?;
    let manager = ObjectsManager::new(Credential::new(
        credentials.access_key,
        credentials.secret_key,
    ));
    let bucket = manager.bucket(bucket_name);
    match bucket.list().limit(1).iter().next() {
        Some(Ok(_)) | None => Ok(()),
        Some(Err(error)) => Err(error.into()),
    }
}

/// 显式请求时执行随机对象的写入、读取和清理，验证完整素材链路。
async fn check_write(provider: &storage_provider::Model, server_key: &str) -> Result<()> {
    let key = format!("oxide-probe/{}", uuid::Uuid::new_v4());
    let content = Bytes::from_static(b"oxide storage probe");
    storage::put(provider, server_key, &key, content.clone(), "text/plain").await?;
    let read_result = storage::get(provider, server_key, &key).await;
    let cleanup_result = storage::delete(provider, server_key, &key).await;
    let received = read_result.context("七牛测试对象读取失败")?;
    cleanup_result.context("七牛测试对象清理失败")?;
    anyhow::ensure!(received == content, "七牛测试对象内容不匹配");
    Ok(())
}

/// SeaORM 只读加载七牛配置，再由 SDK 验证空间访问权限。
#[tokio::main]
async fn main() -> Result<()> {
    let write = env::args().any(|arg| arg == "--write");
    let config = Config::load()?;
    let db = Database::connect(&config.database_url)
        .await
        .context("连接 PostgreSQL 失败")?;
    let providers = storage_provider::Entity::find()
        .filter(storage_provider::Column::Kind.eq(StorageProviderKind::QiniuKodo))
        .all(&db)
        .await
        .context("读取七牛提供商失败")?;
    if providers.is_empty() {
        println!("未找到已保存的七牛 Kodo 提供商");
        return Ok(());
    }
    let mut failed = false;
    for provider in providers {
        match tokio::task::spawn_blocking({
            let server_key = config.comment_hash_key.clone();
            let provider = provider.clone();
            move || check_provider(&provider, &server_key)
        })
        .await
        {
            Ok(Ok(())) => println!("{}: 七牛 SDK 只读请求成功", provider.id),
            _ => {
                println!(
                    "{}: 七牛 SDK 只读请求失败（详情未输出，以免泄露凭据）",
                    provider.id
                );
                failed = true;
            }
        }
        if write {
            match check_write(&provider, &config.comment_hash_key).await {
                Ok(()) => println!("{}: 博客存储适配器上传、读取和清理成功", provider.id),
                _ => {
                    println!(
                        "{}: 博客存储适配器写入链路失败（敏感错误详情未输出）",
                        provider.id
                    );
                    failed = true;
                }
            }
        }
    }
    if failed {
        anyhow::bail!("至少一个七牛空间连接检查失败");
    }
    Ok(())
}
