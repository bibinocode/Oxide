//! 为对象存储提供商增加后台可编辑的连接信息。
use sea_orm_migration::prelude::*;

/// 保留旧环境变量记录的空值兼容迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 扩展连接字段；加密由应用层完成。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("storage_providers"))
                    .add_column(ColumnDef::new(Alias::new("endpoint")).string().null())
                    .add_column(ColumnDef::new(Alias::new("bucket")).string().null())
                    .add_column(ColumnDef::new(Alias::new("region")).string().null())
                    .add_column(
                        ColumnDef::new(Alias::new("encrypted_credentials"))
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    /// 移除本次增加的字段。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("storage_providers"))
                    .drop_column(Alias::new("encrypted_credentials"))
                    .drop_column(Alias::new("region"))
                    .drop_column(Alias::new("bucket"))
                    .drop_column(Alias::new("endpoint"))
                    .to_owned(),
            )
            .await
    }
}
