//! Agent 任务到模型提供商的绑定。
use sea_orm::entity::prelude::*;

/// 任务绑定与模型记录分离，便于同一模型服务多个能力。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "agent_bindings")]
pub struct Model {
    /// 任务名：writing、summary、image 等。
    #[sea_orm(primary_key, auto_increment = false)]
    pub task: String,
    /// 对应提供商标识。
    pub provider_id: String,
}

impl ActiveModelBehavior for ActiveModel {}
