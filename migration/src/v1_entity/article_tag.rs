//! 文章与标签的多对多关联。

use sea_orm::entity::prelude::*;

/// 用组合主键防止同一篇文章重复添加同一标签。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "article_tags")]
pub struct Model {
    /// 关联的文章。
    #[sea_orm(primary_key, auto_increment = false)]
    pub article_id: i64,
    /// 关联的标签。
    #[sea_orm(primary_key, auto_increment = false, indexed)]
    pub tag_id: i64,
    /// 文章关联。
    #[sea_orm(belongs_to, from = "article_id", to = "id")]
    pub article: BelongsTo<super::article::Entity>,
    /// 标签关联。
    #[sea_orm(belongs_to, from = "tag_id", to = "id")]
    pub tag: BelongsTo<super::tag::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
