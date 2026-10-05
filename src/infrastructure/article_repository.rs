//! 使用 SeaORM 实现公开文章读取。

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::collections::HashMap;

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
    cover_asset_id: Option<i64>,
    public_id: uuid::Uuid,
    slug: String,
    title: String,
    summary: Option<String>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    paid_column_public_id: Option<uuid::Uuid>,
    subscriber_only: bool,
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
    paid_column_public_id: Option<uuid::Uuid>,
    subscriber_only: bool,
    rendered_html: String,
    preview_html: Option<String>,
}

impl From<SummaryRow> for ArticleSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            public_id: row.public_id,
            slug: row.slug,
            title: row.title,
            summary: row.summary,
            cover_url: None,
            published_at: row.published_at,
            paid_column_public_id: row.paid_column_public_id,
            subscriber_only: row.subscriber_only,
        }
    }
}

#[async_trait]
impl ArticleRepository for SeaOrmArticleRepository {
    /// 普通写作目录只包含未归属小册的已发布文章。
    /// 小册中的免费与付费章节均由小册目录展示；计数和分页共用过滤条件，
    /// 避免页面过滤造成条目不足或总数不一致。
    async fn list_published(&self, page: u64, per_page: u64) -> Result<ArticlePage> {
        let base = article::Entity::find()
            .filter(article::Column::Status.eq(ArticleStatus::Published))
            .filter(article::Column::PaidColumnPublicId.is_null());
        let total = base.clone().count(&self.db).await?;
        let rows = base
            .select_only()
            .columns([
                article::Column::PublicId,
                article::Column::Slug,
                article::Column::Title,
                article::Column::Summary,
                article::Column::CoverAssetId,
                article::Column::PublishedAt,
                article::Column::PaidColumnPublicId,
                article::Column::SubscriberOnly,
            ])
            .order_by_desc(article::Column::PublishedAt)
            .order_by_desc(article::Column::Id)
            .limit(per_page)
            .offset((page - 1).saturating_mul(per_page))
            .into_model::<SummaryRow>()
            .all(&self.db)
            .await?;

        let ids: Vec<i64> = rows.iter().filter_map(|row| row.cover_asset_id).collect();
        let covers: HashMap<i64, String> = if ids.is_empty() {
            HashMap::new()
        } else {
            asset::Entity::find()
                .filter(asset::Column::Id.is_in(ids))
                .filter(asset::Column::Visibility.eq(AssetVisibility::Public))
                .all(&self.db)
                .await?
                .into_iter()
                .map(|row| (row.id, format!("/media/{}", row.public_id)))
                .collect()
        };
        Ok(ArticlePage {
            items: rows
                .into_iter()
                .map(|row| {
                    let cover_url = row.cover_asset_id.and_then(|id| covers.get(&id).cloned());
                    ArticleSummary {
                        cover_url,
                        ..row.into()
                    }
                })
                .collect(),
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
                article::Column::PaidColumnPublicId,
                article::Column::SubscriberOnly,
                article::Column::RenderedHtml,
                article::Column::PreviewHtml,
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
            cover_url: cover_url.clone(),
            summary: ArticleSummary {
                public_id: row.public_id,
                slug: row.slug,
                title: row.title,
                summary: row.summary,
                cover_url,
                published_at: row.published_at,
                paid_column_public_id: row.paid_column_public_id,
                subscriber_only: row.subscriber_only,
            },
            rendered_html: row.rendered_html,
            preview_html: row.preview_html,
        }))
    }
}
