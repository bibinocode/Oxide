//! 数据库存储的状态枚举。
//!
//! 使用字符串列保存稳定值，避免修改业务状态时重建 PostgreSQL 枚举类型。

use sea_orm::entity::prelude::*;

/// 文章是否已经公开发布。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum ArticleStatus {
    /// 仅管理员可见的草稿。
    #[sea_orm(string_value = "draft")]
    Draft,
    /// 对访客和 RSS 可见的文章。
    #[sea_orm(string_value = "published")]
    Published,
}

/// 匿名评论的审核状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum CommentStatus {
    /// 等待管理员审核。
    #[sea_orm(string_value = "pending")]
    Pending,
    /// 审核通过，可公开展示。
    #[sea_orm(string_value = "approved")]
    Approved,
    /// 审核拒绝，不公开展示。
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

/// 素材是否允许公开访问。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum AssetVisibility {
    /// 草稿素材，仅管理员可预览。
    #[sea_orm(string_value = "private")]
    Private,
    /// 已发布素材，可经本站稳定地址访问。
    #[sea_orm(string_value = "public")]
    Public,
}

/// 已配置对象存储的提供商类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
pub enum StorageProviderKind {
    /// 阿里云对象存储 OSS。
    #[sea_orm(string_value = "aliyun_oss")]
    AliyunOss,
    /// 七牛云 Kodo 对象存储。
    #[sea_orm(string_value = "qiniu_kodo")]
    QiniuKodo,
}

/// Tantivy 索引需要执行的文章变更。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum SearchAction {
    /// 添加或替换已发布文章的索引文档。
    #[sea_orm(string_value = "upsert")]
    Upsert,
    /// 从索引移除文章。
    #[sea_orm(string_value = "delete")]
    Delete,
}

/// 搜索索引任务的处理进度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum SearchJobStatus {
    /// 待后台任务处理。
    #[sea_orm(string_value = "pending")]
    Pending,
    /// 正在处理，可在超时后重新领取。
    #[sea_orm(string_value = "processing")]
    Processing,
    /// 已写入并提交到索引。
    #[sea_orm(string_value = "done")]
    Done,
    /// 处理失败，等待重试或人工检查。
    #[sea_orm(string_value = "failed")]
    Failed,
}
