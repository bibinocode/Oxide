//! 独立备份进程入口：每 72 小时备份一次，也提供单次执行、下载和恢复审计。

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rust_oxide::{
    domain::backup::{INTERVAL_SECONDS, due},
    infrastructure::backup::{self, BackupConfig, QiniuBackupStore},
};
use sea_orm::{ConnectOptions, Database, EntityTrait, PaginatorTrait};
use std::{env, path::Path, sync::Arc, time::Duration};

/// 命令行只接收动作和非敏感文件名，数据库连接信息由环境提供。
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let action = arguments.first().map(String::as_str).unwrap_or("daemon");
    if action == "audit" {
        return audit().await;
    }
    let config = Arc::new(BackupConfig::load()?);
    if action == "status" {
        let state = config.state()?.context("尚未完成第一次备份")?;
        if state.cleanup_pending
            || (Utc::now() - state.backup.created_at).num_seconds() > INTERVAL_SECONDS + 6 * 3600
        {
            bail!("备份已过期或旧备份清理待重试");
        }
        println!("{}", serde_json::to_string(&state)?);
        return Ok(());
    }
    if action == "download" {
        if arguments.len() != 3 {
            bail!("用法：oxide-backup download <对象名> <新文件路径>");
        }
        let mut store = QiniuBackupStore::connect(&config).await?;
        backup::download(&mut store, &arguments[1], Path::new(&arguments[2]))?;
        println!("备份下载和 SHA-256 校验通过");
        return Ok(());
    }
    if action != "daemon" && action != "once" {
        bail!("动作必须是 daemon、once、status、download 或 audit");
    }
    loop {
        if action == "daemon" {
            let state = config.state()?;
            if !state.as_ref().is_some_and(|state| state.cleanup_pending)
                && !due(Utc::now(), state.as_ref())
            {
                let state = state.context("调度状态缺失")?;
                let remaining =
                    INTERVAL_SECONDS - (Utc::now() - state.backup.created_at).num_seconds();
                tokio::time::sleep(Duration::from_secs(remaining.max(1) as u64)).await;
                continue;
            }
        }
        let result = async {
            let mut store = QiniuBackupStore::connect(&config).await?;
            let config = Arc::clone(&config);
            tokio::task::spawn_blocking(move || backup::run(&config, &mut store)).await?
        }
        .await;
        match result {
            Ok(()) => println!("备份上传、完整性验证和旧备份清理完成"),
            Err(error) => {
                let message = format!("{error:#}");
                config.record_failure(&message)?;
                eprintln!("备份任务失败：{message}");
                if action == "once" {
                    return Err(error);
                }
                // 保持上一份备份及成功时间，失败后每小时重试；不把失败计为新周期。
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        }
        if action == "once" {
            return Ok(());
        }
    }
}

/// 对恢复目标只执行 SeaORM 计数，便于隔离恢复演练核对全部业务表。
async fn audit() -> Result<()> {
    use rust_oxide::entity::*;
    let mut options = ConnectOptions::new(env::var("DATABASE_URL").context("缺少 DATABASE_URL")?);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|_| anyhow::anyhow!("恢复审计数据库连接失败"))?;
    let mut counts = serde_json::Map::new();
    macro_rules! count { ($($entity:ident),+ $(,)?) => { $( counts.insert(stringify!($entity).into(), $entity::Entity::find().count(&db).await?.into()); )+ }; }
    count!(
        admin_user,
        agent_binding,
        agent_provider,
        article,
        article_category,
        article_revision,
        article_tag,
        asset,
        category,
        column_order,
        column_subscription,
        comment,
        paid_column,
        reader_user,
        search_job,
        site_setting,
        storage_provider,
        tag
    );
    println!("{}", serde_json::Value::Object(counts));
    Ok(())
}
