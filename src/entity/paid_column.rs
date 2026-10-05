//! 可单独购买的文章专栏。

use sea_orm::entity::prelude::*;

/// 专栏价格以人民币分存储，已生成订单保存价格快照。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "paid_columns")]
pub struct Model {
    /// 数据库内部主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 是否在公开目录展示；隐藏不删除文章或历史权益。
    pub visible: bool,
    /// 对外公开的随机标识。
    #[sea_orm(unique)]
    pub public_id: Uuid,
    /// 专栏 URL 标识。
    #[sea_orm(unique)]
    pub slug: String,
    /// 展示名称。
    pub title: String,
    /// 展示简介，不存放受保护正文。
    #[sea_orm(column_type = "Text")]
    pub description: String,
    /// 当前售价，单位为分。
    pub price_cents: i32,
    /// 创建时间。
    pub created_at: DateTimeUtc,
    /// 最近更新时间。
    pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
