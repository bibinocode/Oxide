//! 文章索引更新任务。

use sea_orm::entity::prelude::*;

use super::status::{SearchAction, SearchJobStatus};

/// 与文章变更一起提交的持久任务；删除文章后任务仍须保留。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "search_jobs")]
pub struct Model {
    /// 任务主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 待更新文章的 ID；故意不建外键，以支持文章删除后的索引清理。
    #[sea_orm(indexed)]
    pub article_id: i64,
    /// 更新或删除索引文档。
    pub action: SearchAction,
    /// 领取和重试状态。
    #[sea_orm(indexed)]
    pub status: SearchJobStatus,
    /// 已尝试执行次数。
    pub attempts: i32,
    /// 任务创建时间。
    pub created_at: DateTimeUtc,
    /// 最近一次状态变更时间。
    pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
