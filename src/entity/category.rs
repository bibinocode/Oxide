//! 文章分类。

use sea_orm::entity::prelude::*;

/// 用于组织文章的分类；首版允许一篇文章归入多个分类。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "categories")]
pub struct Model {
    /// 分类主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 是否在公开目录展示；隐藏不删除文章或历史权益。
    pub visible: bool,
    /// 显示名称。
    pub name: String,
    /// 分类 URL 使用的唯一标识。
    #[sea_orm(unique)]
    pub slug: String,
    /// 与文章的关联行。
    #[sea_orm(has_many)]
    pub article_categories: HasMany<super::article_category::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
