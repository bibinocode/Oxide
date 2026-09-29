//! 匿名评论及审核状态。

use sea_orm::entity::prelude::*;

use super::status::CommentStatus;

/// 匿名评论；不保存访客的邮箱明文。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    /// 评论主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 所属文章。
    #[sea_orm(indexed)]
    pub article_id: i64,
    /// 回复目标；首版仅允许回复顶层评论。
    #[sea_orm(indexed)]
    pub parent_id: Option<i64>,
    /// 访客填写的公开昵称。
    pub nickname: String,
    /// 规范化邮箱的带密钥摘要，仅供稳定头像和滥用控制。
    pub email_hash: String,
    /// 评论正文；公开渲染时还需要转义或净化。
    #[sea_orm(column_type = "Text")]
    pub body: String,
    /// 待审核、通过或拒绝。
    #[sea_orm(indexed)]
    pub status: CommentStatus,
    /// 提交时间。
    pub created_at: DateTimeUtc,
    /// 审核时间；未审核时为空。
    pub reviewed_at: Option<DateTimeUtc>,
    /// 所属文章。
    #[sea_orm(belongs_to, from = "article_id", to = "id")]
    pub article: BelongsTo<super::article::Entity>,
    /// 被回复的评论。
    #[sea_orm(self_ref, relation_enum = "Parent", from = "parent_id", to = "id")]
    pub parent: BelongsTo<Option<Entity>>,
}

impl ActiveModelBehavior for ActiveModel {}
