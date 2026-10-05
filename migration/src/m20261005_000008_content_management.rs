//! 内容可见性与评论 Agent 审核元数据；仅追加字段，不改写现有发布与审核结果。
use sea_orm_migration::prelude::*;

/// 为内容管理追加向后兼容的结构。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 现有小册、分类和标签保持公开；历史评论不伪造 Agent 审核记录。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in ["paid_columns", "categories", "tags"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new(table))
                        .add_column(
                            ColumnDef::new(Alias::new("visible"))
                                .boolean()
                                .not_null()
                                .default(true),
                        )
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("comments"))
                    .add_column(ColumnDef::new(Alias::new("agent_review")).json_binary())
                    .add_column(
                        ColumnDef::new(Alias::new("review_version"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(ColumnDef::new(Alias::new("review_source")).string_len(16))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
    /// 回退只移除本次追加的管理字段，不删除业务记录。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("comments"))
                    .drop_column(Alias::new("agent_review"))
                    .drop_column(Alias::new("review_version"))
                    .drop_column(Alias::new("review_source"))
                    .to_owned(),
            )
            .await?;
        for table in ["paid_columns", "categories", "tags"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new(table))
                        .drop_column(Alias::new("visible"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
