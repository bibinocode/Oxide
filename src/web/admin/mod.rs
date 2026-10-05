//! 管理员认证、内容维护和存储配置接口。

pub mod agent;
pub mod auth;
pub mod columns;
pub mod comments;
pub mod content;
pub mod cover;
pub mod notion;
pub mod preview;
pub mod settings;
pub mod storage;
pub mod taxonomy;

/// 内容可见性修改：不删除文章、标签关联或历史购买权益。
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct ContentVisibilityInput {
    /// true 为公开，false 为下架或隐藏。
    pub visible: bool,
}
