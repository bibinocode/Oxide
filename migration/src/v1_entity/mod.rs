//! 首版迁移专用的不可变实体快照。
//!
//! 这里保留添加外部 UUID 之前的列定义；新业务代码使用根 crate 的实体。

pub mod admin_user;
pub mod article;
pub mod article_category;
pub mod article_revision;
pub mod article_tag;
pub mod asset;
pub mod category;
pub mod comment;
pub mod search_job;
pub mod site_setting;
pub mod status;
pub mod storage_provider;
pub mod tag;
