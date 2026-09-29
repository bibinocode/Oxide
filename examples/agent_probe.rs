//! 只读检查已保存的 Agent 模型类型与任务绑定，不输出密钥。

use anyhow::{Context, Result};
use rust_oxide::{
    config::Config,
    entity::{agent_binding, agent_provider},
};
use sea_orm::{Database, EntityTrait, QueryOrder};

/// 输出非敏感配置，帮助确认后续适配器需要调用的协议。
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let db = Database::connect(&config.database_url)
        .await
        .context("连接 PostgreSQL 失败")?;
    let providers = agent_provider::Entity::find()
        .order_by_asc(agent_provider::Column::Id)
        .all(&db)
        .await?;
    let bindings = agent_binding::Entity::find()
        .order_by_asc(agent_binding::Column::Task)
        .all(&db)
        .await?;
    for provider in providers {
        let host = url::Url::parse(&provider.base_url)?
            .host_str()
            .unwrap_or("未知域名")
            .to_owned();
        println!(
            "{}: adapter={}, capability={}, model={}, host={}, key_configured={}, enabled={}",
            provider.id,
            provider.adapter,
            provider.capability,
            provider.model_id,
            host,
            !provider.encrypted_api_key.is_empty(),
            provider.enabled,
        );
    }
    for binding in bindings {
        println!("task={} -> {}", binding.task, binding.provider_id);
    }
    Ok(())
}
