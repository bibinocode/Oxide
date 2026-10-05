//! 文章正文、发布状态和公开地址。

use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};

use super::status::ArticleStatus;

/// 博客文章；Tiptap 原始文档与已净化的公开 HTML 同时保存。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    /// 自增主键，供关联表和搜索索引引用。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 对外 API 使用的随机标识；内部关联仍使用自增主键。
    #[sea_orm(unique)]
    pub public_id: Uuid,
    /// 永久链接中的唯一标识。
    #[sea_orm(unique)]
    pub slug: String,
    /// 文章标题。
    pub title: String,
    /// 列表、SEO 和 RSS 使用的摘要。
    #[sea_orm(column_type = "Text")]
    pub summary: Option<String>,
    /// 经服务端校验的 Tiptap JSON 文档。
    pub document: Json,
    /// 由文档生成并净化的 HTML，仅发布后公开。
    #[sea_orm(column_type = "Text")]
    pub rendered_html: String,
    /// 草稿或发布状态。
    #[sea_orm(indexed)]
    pub status: ArticleStatus,
    /// 可选封面素材。
    pub cover_asset_id: Option<i64>,
    /// 首次发布时间；草稿为空。
    #[sea_orm(indexed)]
    pub published_at: Option<DateTimeUtc>,
    /// 创建时间。
    pub created_at: DateTimeUtc,
    /// 最近一次编辑时间。
    pub updated_at: DateTimeUtc,
    /// 已导入的 Notion 页面标识；每页最多对应一篇文章。
    #[sea_orm(unique)]
    pub notion_page_id: Option<Uuid>,
    /// 上次成功同步时 Notion 页面的编辑时间。
    pub notion_last_edited_at: Option<DateTimeUtc>,
    /// 上次成功同步到本站的时间，用于检测后续本地编辑。
    pub notion_synced_at: Option<DateTimeUtc>,
    /// 所属付费专栏；普通公开文章为空。
    pub paid_column_public_id: Option<Uuid>,
    /// 是否仅向已购读者展示完整正文。
    pub subscriber_only: bool,
    /// 作者提供的公开试看文档，不含受保护正文。
    pub preview_document: Option<Json>,
    /// 服务端从试看文档生成的公开 HTML。
    #[sea_orm(column_type = "Text")]
    pub preview_html: Option<String>,
    /// 封面素材的可选关联。
    #[sea_orm(belongs_to, from = "cover_asset_id", to = "id")]
    pub cover_asset: BelongsTo<Option<super::asset::Entity>>,
    /// 可恢复的文章修订。
    #[sea_orm(has_many)]
    pub revisions: HasMany<super::article_revision::Entity>,
    /// 文章下的匿名评论。
    #[sea_orm(has_many)]
    pub comments: HasMany<super::comment::Entity>,
    /// 分类关联行。
    #[sea_orm(has_many)]
    pub article_categories: HasMany<super::article_category::Entity>,
    /// 标签关联行。
    #[sea_orm(has_many)]
    pub article_tags: HasMany<super::article_tag::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    /// 新建文章时自动生成不可预测的外部标识。
    fn new() -> Self {
        Self {
            id: NotSet,
            public_id: Set(Uuid::new_v4()),
            slug: NotSet,
            title: NotSet,
            summary: NotSet,
            document: NotSet,
            rendered_html: NotSet,
            status: NotSet,
            cover_asset_id: NotSet,
            published_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
            notion_page_id: NotSet,
            notion_last_edited_at: NotSet,
            notion_synced_at: NotSet,
            paid_column_public_id: Set(None),
            subscriber_only: Set(false),
            preview_document: Set(None),
            preview_html: Set(None),
        }
    }

    /// 兼容通过默认模型构造的插入，保证外部 ID 始终存在。
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 新文章的默认模型可安全构造且公开 ID 不可预测。
    #[test]
    fn default_article_has_public_uuid() {
        let first: ActiveModel = Default::default();
        let second: ActiveModel = Default::default();
        assert_ne!(first.public_id.unwrap(), second.public_id.unwrap());
    }
}
