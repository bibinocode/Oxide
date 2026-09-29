//! 站点级公开配置。

use sea_orm::entity::prelude::*;

/// 单行站点设置；初始化流程固定使用 ID 1。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "site_settings")]
pub struct Model {
    /// 固定为 1 的主键。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// 站点名称，供页面标题和 RSS 使用。
    pub site_name: String,
    /// 站点简介。
    #[sea_orm(column_type = "Text")]
    pub description: Option<String>,
    /// 公开基础 URL，用于生成绝对链接。
    pub base_url: String,
    /// 公开联系方式、作品集、服务模块与页脚配置。
    #[sea_orm(column_type = "JsonBinary")]
    pub presentation: Option<Json>,
    /// 新上传素材使用的存储实例。
    pub active_provider_id: Option<String>,
    /// 最近一次设置变更时间。
    pub updated_at: DateTimeUtc,
    /// 当前上传提供商；为空时不允许上传。
    #[sea_orm(belongs_to, from = "active_provider_id", to = "id")]
    pub active_provider: BelongsTo<Option<super::storage_provider::Entity>>,
}

impl ActiveModelBehavior for ActiveModel {}
