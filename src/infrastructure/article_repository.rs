//! 使用 SeaORM 实现公开文章读取。

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::{
    domain::article::{ArticleDetail, ArticlePage, ArticleRepository, ArticleSummary},
    entity::{
        article, asset,
        status::{ArticleStatus, AssetVisibility},
    },
};

/// 持有 SeaORM 连接池的文章仓储。
pub struct SeaOrmArticleRepository {
    db: DatabaseConnection,
}

impl SeaOrmArticleRepository {
    /// 共享连接池句柄，克隆不会额外建立连接。
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

/// 列表仅投影公开字段，避免加载正文 JSON 和 HTML。
#[derive(FromQueryResult)]
struct SummaryRow {
    public_id: uuid::Uuid,
    slug: String,
    title: String,
    summary: Option<String>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 详情额外读取公开 HTML，不加载 Tiptap 文档和内部关联字段。
#[derive(FromQueryResult)]
struct DetailRow {
    cover_asset_id: Option<i64>,
    public_id: uuid::Uuid,
    slug: String,
    title: String,
    summary: Option<String>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    rendered_html: String,
}

impl From<SummaryRow> for ArticleSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            public_id: row.public_id,
            slug: row.slug,
            title: row.title,
            summary: row.summary,
            published_at: row.published_at,
        }
    }
}

#[async_trait]
impl ArticleRepository for SeaOrmArticleRepository {
    /// 查询状态过滤在数据库执行，分页偏移使用已验证的页码。
    async fn list_published(&self, page: u64, per_page: u64) -> Result<ArticlePage> {
        let base =
            article::Entity::find().filter(article::Column::Status.eq(ArticleStatus::Published));
        let total = base.clone().count(&self.db).await?;
        let rows = base
            .select_only()
            .columns([
                article::Column::PublicId,
                article::Column::Slug,
                article::Column::Title,
                article::Column::Summary,
                article::Column::PublishedAt,
            ])
            .order_by_desc(article::Column::PublishedAt)
            .order_by_desc(article::Column::Id)
            .limit(per_page)
            .offset((page - 1).saturating_mul(per_page))
            .into_model::<SummaryRow>()
            .all(&self.db)
            .await?;

        Ok(ArticlePage {
            items: rows.into_iter().map(Into::into).collect(),
            total,
        })
    }

    /// 详情查询不暴露实体本身，避免意外序列化内部字段。
    async fn get_published(&self, slug: &str) -> Result<Option<ArticleDetail>> {
        let row = article::Entity::find()
            .filter(article::Column::Slug.eq(slug))
            .filter(article::Column::Status.eq(ArticleStatus::Published))
            .select_only()
            .columns([
                article::Column::PublicId,
                article::Column::Slug,
                article::Column::Title,
                article::Column::Summary,
                article::Column::PublishedAt,
                article::Column::RenderedHtml,
                article::Column::CoverAssetId,
            ])
            .into_model::<DetailRow>()
            .one(&self.db)
            .await?;
        let cover_url = if let Some(id) = row.as_ref().and_then(|row| row.cover_asset_id) {
            asset::Entity::find_by_id(id)
                .filter(asset::Column::Visibility.eq(AssetVisibility::Public))
                .one(&self.db)
                .await?
                .map(|asset| format!("/media/{}", asset.public_id))
        } else {
            None
        };
        Ok(row.map(|row| ArticleDetail {
            cover_url,
            summary: ArticleSummary {
                public_id: row.public_id,
                slug: row.slug,
                title: row.title,
                summary: row.summary,
                published_at: row.published_at,
            },
            rendered_html: row.rendered_html,
        }))
    }
}
