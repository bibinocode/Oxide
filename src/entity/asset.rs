//! 图片等上传素材的元数据。

use sea_orm::Set;
use sea_orm::entity::prelude::*;

use super::status::AssetVisibility;

/// 素材记录；对象内容始终位于其所属的存储提供商。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "assets")]
pub struct Model {
    /// 内部关联主键，不出现在对外素材地址中。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// `/media/{public_id}` 使用的随机标识。
    #[sea_orm(unique)]
    pub public_id: Uuid,
    /// 创建素材时实际使用的提供商。
    #[sea_orm(indexed, unique_key = "provider_object")]
    pub provider_id: String,
    /// 提供商内的对象键，与提供商标识组合后唯一。
    #[sea_orm(unique_key = "provider_object")]
    pub object_key: String,
    /// 服务端验证后的 MIME 类型。
    pub mime_type: String,
    /// 对象大小，以字节计。
    pub size_bytes: i64,
    /// 图片宽度；非图片素材为空。
    pub width: Option<i32>,
    /// 图片高度；非图片素材为空。
    pub height: Option<i32>,
    /// 决定素材是否可从公开地址访问。
    pub visibility: AssetVisibility,
    /// 上传时间。
    pub created_at: DateTimeUtc,
    /// 素材所属的存储实例。
    #[sea_orm(belongs_to, from = "provider_id", to = "id")]
    pub provider: BelongsTo<super::storage_provider::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    /// 新素材自动取得稳定的外部标识。
    fn new() -> Self {
        Self {
            public_id: Set(Uuid::new_v4()),
            ..Default::default()
        }
    }

    /// 插入前补齐外部 ID，兼容默认模型构造方式。
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.is_not_set(Column::PublicId) {
            self.public_id = Set(Uuid::new_v4());
        }
        Ok(self)
    }
}
