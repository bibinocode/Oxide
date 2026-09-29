//! 匿名评论及审核状态。

use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};

use super::status::CommentStatus;

/// 匿名评论；不保存访客的邮箱明文。
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    /// 评论主键。
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 对外评论和回复接口使用的随机标识。
    #[sea_orm(unique)]
    pub public_id: Uuid,
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

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    /// 新评论自动取得不暴露数据库序号的外部标识。
    fn new() -> Self {
        Self {
            id: NotSet,
            public_id: Set(Uuid::new_v4()),
            article_id: NotSet,
            parent_id: NotSet,
            nickname: NotSet,
            email_hash: NotSet,
            body: NotSet,
            status: NotSet,
            created_at: NotSet,
            reviewed_at: NotSet,
        }
    }

    /// 插入前补齐外部 ID，避免匿名评论遗漏公开标识。
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

    /// 自关联评论的模型构造不得触发默认值递归。
    #[test]
    fn new_comment_has_public_uuid() {
        let model = ActiveModel::new();
        assert!(matches!(model.public_id, sea_orm::ActiveValue::Set(_)));
    }
}
