//! 文章编辑历史。

use sea_orm::entity::prelude::*;

/// 已保存的 Tiptap 文档快照，供管理员恢复历史内容。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "article_revisions")]
pub struct Model {
    /// 修订记录主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 被修订的文章。
    #[sea_orm(indexed)]
    pub article_id: i64,
    /// 保存当时的 Tiptap JSON 文档。
    pub document: Json,
    /// 保存时间。
    pub saved_at: DateTimeUtc,
    /// 所属文章。
    #[sea_orm(belongs_to, from = "article_id", to = "id")]
    pub article: BelongsTo<super::article::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
