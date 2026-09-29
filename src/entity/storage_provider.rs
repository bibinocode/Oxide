//! 对象存储配置的非敏感元数据。

use sea_orm::entity::prelude::*;

use super::status::StorageProviderKind;

/// 存储提供商实例；密钥由环境或密钥管理系统提供。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "storage_providers")]
pub struct Model {
    /// 稳定标识，供素材引用；切换当前提供商时不可重用旧标识。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// 提供商实现类型。
    pub kind: StorageProviderKind,
    /// 管理端显示的名称。
    pub name: String,
    /// 公开访问域名，可指向 CDN。
    pub public_base_url: String,
    /// S3 兼容端点，旧记录可继续从环境变量读取。
    pub endpoint: Option<String>,
    /// 对象存储空间名。
    pub bucket: Option<String>,
    /// 服务端区域标识。
    pub region: Option<String>,
    /// 经 AES-GCM 加密的访问凭据；绝不进入 API 响应。
    pub encrypted_credentials: Option<String>,
    /// 是否可接受新上传；停用后仍需保留旧素材读取配置。
    pub upload_enabled: bool,
    /// 配置创建时间。
    pub created_at: DateTimeUtc,
    /// 此提供商保存的素材。
    #[sea_orm(has_many)]
    pub assets: HasMany<super::asset::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
