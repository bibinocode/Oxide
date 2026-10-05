//! 博客数据库的 SeaORM 结构化迁移。
//!
//! 首版迁移使用冻结的实体快照；后续结构变更只通过新迁移追加。

use sea_orm_migration::prelude::*;

mod m20260928_000001_initial;
mod m20260928_000002_public_ids;
mod m20260928_000003_site_presentation;
mod m20260928_000004_provider_credentials;
mod m20260928_000005_agent_core;
mod m20260929_000006_notion_sync;
mod m20260930_000007_paid_columns;
mod m20261005_000008_content_management;
mod v1_entity;

/// 按版本顺序注册数据库迁移。
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    /// 返回本项目全部迁移；SeaORM 负责记录已执行版本。
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260928_000001_initial::Migration),
            Box::new(m20260928_000002_public_ids::Migration),
            Box::new(m20260928_000003_site_presentation::Migration),
            Box::new(m20260928_000004_provider_credentials::Migration),
            Box::new(m20260928_000005_agent_core::Migration),
            Box::new(m20260929_000006_notion_sync::Migration),
            Box::new(m20260930_000007_paid_columns::Migration),
            Box::new(m20261005_000008_content_management::Migration),
        ]
    }
}
