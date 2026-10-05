//! 外部资源的具体适配器。

pub mod agent;
pub mod agent_image;
pub mod agent_skills;
pub mod article_repository;
pub mod notion;
pub mod notion_media;
pub mod search;
pub mod secrets;
pub mod storage;
pub mod web_search;
pub mod webfetch;

pub mod backup;

/// 持久化待审评论的后台 Agent 审核调度。
pub mod comment_moderation;
