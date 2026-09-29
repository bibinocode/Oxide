//! 为文章记录 Notion 来源和最近同步时间，避免重复导入。

use sea_orm_migration::prelude::*;

/// Notion 文章同步所需的来源字段。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 新增可空来源字段，旧文章无需回填。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("articles"))
                    .add_column(ColumnDef::new(Alias::new("notion_page_id")).uuid())
                    .add_column(
                        ColumnDef::new(Alias::new("notion_last_edited_at"))
                            .timestamp_with_time_zone(),
                    )
                    .add_column(
                        ColumnDef::new(Alias::new("notion_synced_at")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-articles-notion_page_id")
                    .table(Alias::new("articles"))
                    .col(Alias::new("notion_page_id"))
                    .unique()
                    .to_owned(),
            )
            .await
    }

    /// 回滚来源字段及其索引，不改变文章正文。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-articles-notion_page_id")
                    .table(Alias::new("articles"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("articles"))
                    .drop_column(Alias::new("notion_page_id"))
                    .drop_column(Alias::new("notion_last_edited_at"))
                    .drop_column(Alias::new("notion_synced_at"))
                    .to_owned(),
            )
            .await
    }
}
