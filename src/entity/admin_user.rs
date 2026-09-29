//! 唯一站点管理员的登录信息。

use sea_orm::entity::prelude::*;

/// 管理员账户；首版由初始化流程创建，不开放注册。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "admin_users")]
pub struct Model {
    /// 自增主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 登录名，数据库保证唯一。
    #[sea_orm(unique)]
    pub username: String,
    /// Argon2 密码摘要，不保存明文密码。
    pub password_hash: String,
    /// 创建时间，统一使用 UTC。
    pub created_at: DateTimeUtc,
    /// 最近一次资料变更时间。
    pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
