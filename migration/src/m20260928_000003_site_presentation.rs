//! 为站点增加可扩展的公开模块配置，不修改既有数据。
use sea_orm_migration::prelude::*;

/// 公开展示配置迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 空值使用应用默认配置，保证旧站点行为稳定。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("site_settings"))
                    .add_column(
                        ColumnDef::new(Alias::new("presentation"))
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    /// 回滚仅移除本次增加的配置列。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("site_settings"))
                    .drop_column(Alias::new("presentation"))
                    .to_owned(),
            )
            .await
    }
}
