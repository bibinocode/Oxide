//! Agent 核心使用的模型提供商配置。
use sea_orm::entity::prelude::*;

/// 独立于站点设置的模型提供商；密钥只以认证密文保存。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "agent_providers")]
pub struct Model {
    /// 管理端稳定标识。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// 协议适配器：openai_compatible 或 jimeng。
    pub adapter: String,
    /// 展示名称。
    pub name: String,
    /// HTTPS API 基础地址。
    pub base_url: String,
    /// 提供商实际模型 ID。
    pub model_id: String,
    /// 模型能力：text 或 image。
    pub capability: String,
    /// 加密后的 API key，管理员读取时不可回传。
    pub encrypted_api_key: String,
    /// 管理端启用状态。
    pub enabled: bool,
    /// 创建时间。
    pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
