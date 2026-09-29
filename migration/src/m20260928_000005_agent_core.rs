//! Agent 模型注册和任务绑定，不与站点展示配置耦合。
use sea_orm_migration::prelude::*;

/// Agent 核心持久化迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 创建提供商及绑定表，使用 SeaQuery 结构化语句。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_providers"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("adapter")).string().not_null())
                    .col(ColumnDef::new(Alias::new("name")).string().not_null())
                    .col(ColumnDef::new(Alias::new("base_url")).string().not_null())
                    .col(ColumnDef::new(Alias::new("model_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("capability")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("encrypted_api_key"))
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("enabled")).boolean().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_bindings"))
                    .col(
                        ColumnDef::new(Alias::new("task"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("provider_id"))
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("agent_bindings"), Alias::new("provider_id"))
                            .to(Alias::new("agent_providers"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    /// 先删除引用表，再删除提供商表。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("agent_bindings")).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("agent_providers"))
                    .to_owned(),
            )
            .await
    }
}
