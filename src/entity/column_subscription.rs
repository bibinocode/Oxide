//! 读者对专栏的已购访问权。

use sea_orm::entity::prelude::*;

/// 一名读者对一个专栏最多有一条永久订阅记录。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "column_subscriptions")]
pub struct Model {
    /// 数据库内部主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 已付款读者。
    pub reader_id: i64,
    /// 已购买专栏。
    pub paid_column_id: i64,
    /// 授权建立时间。
    pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
