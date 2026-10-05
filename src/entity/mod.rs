//! SeaORM 2.0 数据实体及关联。
//!
//! 实体只描述持久化结构；输入校验、权限和状态流转由后续业务层负责。

pub mod admin_user;
pub mod agent_binding;
pub mod agent_provider;
pub mod article;
pub mod article_category;
pub mod article_revision;
pub mod article_tag;
pub mod asset;
pub mod category;
pub mod column_order;
pub mod column_subscription;
pub mod comment;
pub mod paid_column;
pub mod reader_user;
pub mod search_job;
pub mod site_setting;
pub mod status;
pub mod storage_provider;
pub mod tag;
