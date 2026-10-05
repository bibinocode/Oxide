//! 微信支付订单及价格快照。

use sea_orm::entity::prelude::*;

/// 订单只允许可信的支付回调由待支付转换为已支付。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "column_orders")]
pub struct Model {
    /// 商户订单号，32 位十六进制 UUID。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// 下单读者。
    pub reader_id: i64,
    /// 购买专栏。
    pub paid_column_id: i64,
    /// 下单时价格快照，单位为分。
    pub amount_cents: i32,
    /// pending 或 paid。
    pub status: String,
    /// 微信支付流水号，用于审计与幂等校验。
    #[sea_orm(unique)]
    pub wechat_transaction_id: Option<String>,
    /// 下单时间。
    pub created_at: DateTimeUtc,
    /// 微信确认付款时间。
    pub paid_at: Option<DateTimeUtc>,
}

impl ActiveModelBehavior for ActiveModel {}
