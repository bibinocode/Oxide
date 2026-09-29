//! 文章与分类的多对多关联。

use sea_orm::entity::prelude::*;

/// 用组合主键防止同一篇文章重复加入同一分类。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "article_categories")]
pub struct Model {
    /// 关联的文章。
    #[sea_orm(primary_key, auto_increment = false)]
    pub article_id: i64,
    /// 关联的分类。
    #[sea_orm(primary_key, auto_increment = false, indexed)]
    pub category_id: i64,
    /// 文章关联。
    #[sea_orm(belongs_to, from = "article_id", to = "id")]
    pub article: BelongsTo<super::article::Entity>,
    /// 分类关联。
    #[sea_orm(belongs_to, from = "category_id", to = "id")]
    pub category: BelongsTo<super::category::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
