//! 文章标签。

use sea_orm::entity::prelude::*;

/// 描述文章主题的标签。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    /// 标签主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 是否在公开目录展示；隐藏不删除文章或历史权益。
    pub visible: bool,
    /// 显示名称。
    pub name: String,
    /// 标签 URL 使用的唯一标识。
    #[sea_orm(unique)]
    pub slug: String,
    /// 与文章的关联行。
    #[sea_orm(has_many)]
    pub article_tags: HasMany<super::article_tag::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
