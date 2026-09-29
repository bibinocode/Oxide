//! 公开文章读取模型和仓储接口。

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub mod render;

/// 公开列表分页上限，限制数据库查询和响应大小。
pub const MAX_PAGE_SIZE: u64 = 50;

/// 公开列表所需的文章字段，不包含正文和内部主键。
#[derive(Clone, Debug)]
pub struct ArticleSummary {
    /// 对外稳定标识。
    pub public_id: Uuid,
    /// 永久链接标识。
    pub slug: String,
    /// 展示标题。
    pub title: String,
    /// 可选摘要。
    pub summary: Option<String>,
    /// 首次发布时间。
    pub published_at: Option<DateTime<Utc>>,
}

/// 文章详情在列表字段之外包含净化后的公开 HTML。
#[derive(Clone, Debug)]
pub struct ArticleDetail {
    /// 列表和详情共用的字段。
    pub summary: ArticleSummary,
    /// 服务端保存的公开 HTML。
    pub rendered_html: String,
    /// 已公开封面的稳定地址。
    pub cover_url: Option<String>,
}

/// 已发布文章的分页结果。
#[derive(Clone, Debug)]
pub struct ArticlePage {
    /// 当前页文章。
    pub items: Vec<ArticleSummary>,
    /// 已发布文章总数。
    pub total: u64,
}

/// 业务层依赖的文章读取能力，测试可替换为内存实现。
#[async_trait]
pub trait ArticleRepository: Send + Sync {
    /// 按发布日期倒序读取已发布文章。
    async fn list_published(&self, page: u64, per_page: u64) -> Result<ArticlePage>;

    /// 按 slug 读取已发布文章；草稿与不存在均返回 None。
    async fn get_published(&self, slug: &str) -> Result<Option<ArticleDetail>>;
}
