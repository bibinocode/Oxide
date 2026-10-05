//! 读者账号；密码只保存 Argon2 摘要。

use sea_orm::entity::prelude::*;

/// 读者身份与管理员身份严格分离。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "reader_users")]
pub struct Model {
    /// 数据库内部主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 对外随机标识。
    #[sea_orm(unique)]
    pub public_id: Uuid,
    /// 唯一登录名称。
    #[sea_orm(unique)]
    pub username: String,
    /// Argon2 密码摘要。
    #[sea_orm(column_type = "Text")]
    pub password_hash: String,
    /// 注册时间。
    pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
