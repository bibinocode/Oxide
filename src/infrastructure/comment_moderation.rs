//! 评论自动审核：单次只处理一条，Redis 短租约避免多个 API 实例重复调用模型。
use crate::{
    agent::{
        comment_review::{self, Decision},
        context::AgentContext,
    },
    domain::agent::AgentTask,
    entity::{agent_binding, article, comment, status::CommentStatus},
    infrastructure::agent,
};
use anyhow::Result;
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// 在自动结果写回时重新校验人工版本；隐藏、拒绝、通过和删除都不能被旧调用覆盖。
pub async fn finish(
    db: &DatabaseConnection,
    id: i64,
    version: i32,
    status: CommentStatus,
    review: serde_json::Value,
) -> Result<bool> {
    let result = comment::Entity::update_many()
        .col_expr(
            comment::Column::Status,
            sea_orm::sea_query::Expr::value(status),
        )
        .col_expr(
            comment::Column::AgentReview,
            sea_orm::sea_query::Expr::value(review),
        )
        .col_expr(
            comment::Column::ReviewSource,
            sea_orm::sea_query::Expr::value("agent"),
        )
        .col_expr(
            comment::Column::ReviewedAt,
            sea_orm::sea_query::Expr::value(Utc::now()),
        )
        .filter(comment::Column::Id.eq(id))
        .filter(comment::Column::ReviewVersion.eq(version))
        .filter(comment::Column::Status.eq(CommentStatus::Pending))
        .filter(comment::Column::ReviewedAt.is_null())
        .filter(comment::Column::AgentReview.is_null())
        .exec(db)
        .await?;
    Ok(result.rows_affected == 1)
}

/// 只有明确绑定评论审核模型才启用自动任务；其他写作模型不隐式接管评论。
async fn process_one(db: &DatabaseConnection, redis: &redis::Client, key: &str) -> Result<bool> {
    if agent_binding::Entity::find_by_id(AgentTask::CommentReview.as_str())
        .one(db)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    let Some(provider) = agent::text_provider(db, AgentTask::CommentReview).await? else {
        return Ok(false);
    };
    let Some(row) = comment::Entity::find()
        .filter(comment::Column::Status.eq(CommentStatus::Pending))
        .filter(comment::Column::ReviewedAt.is_null())
        .filter(comment::Column::AgentReview.is_null())
        .order_by_asc(comment::Column::Id)
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    let mut connection = redis.get_multiplexed_async_connection().await?;
    // 不提前释放短租约，避免人工重提和进程中断产生同一条评论的重叠请求。
    let locked: Option<String> = redis::cmd("SET")
        .arg(format!("comment-agent:{}", row.public_id))
        .arg("processing")
        .arg("NX")
        .arg("EX")
        .arg(120)
        .query_async(&mut connection)
        .await?;
    if locked.is_none() {
        return Ok(false);
    }
    let Some(article) = article::Entity::find_by_id(row.article_id).one(db).await? else {
        return Ok(false);
    };
    let parent = if let Some(id) = row.parent_id {
        comment::Entity::find_by_id(id).one(db).await?
    } else {
        None
    };
    // 不向模型发送邮箱摘要、存储密钥或未公开文章全文；不给 Skill 和网络工具授权。
    let response = agent::generate_text(
        &provider,
        key,
        &AgentContext::restricted(comment_review::SYSTEM),
        comment_review::prompt(
            &article.title,
            &row.nickname,
            &row.body,
            parent.as_ref().map(|p| p.body.as_str()),
        ),
        comment_review::OPTIONS,
    )
    .await;
    let (status, review) = match response.and_then(|text| comment_review::validate(&text)) {
        Ok(verdict) => {
            let status = match verdict.decision {
                Decision::Approved => CommentStatus::Approved,
                Decision::Rejected => CommentStatus::Rejected,
                Decision::Manual => CommentStatus::Pending,
            };
            (
                status,
                json!({"decision":verdict.decision,"reason":verdict.reason,"provider_id":provider.id,"model_id":provider.model_id,"at":Utc::now()}),
            )
        }
        Err(_) => {
            // 提供商错误可能包含请求正文或签名；日志和审核记录只写通用原因。
            tracing::warn!(comment_id = %row.public_id, "评论 Agent 审核未完成，保留人工待审");
            (
                CommentStatus::Pending,
                json!({"decision":"error","reason":"模型调用失败或审核输出无效，请人工审核或重新提交 Agent","provider_id":provider.id,"model_id":provider.model_id,"at":Utc::now()}),
            )
        }
    };
    finish(db, row.id, row.review_version, status, review).await
}

/// 连续轮询不会堆积任务或一次加载整批正文；模型失败转人工，不反复自动消耗调用额度。
pub async fn run_worker(db: DatabaseConnection, redis: redis::Client, key: Arc<str>) {
    loop {
        match process_one(&db, &redis, &key).await {
            Ok(true) => continue,
            Ok(false) => {}
            Err(_) => tracing::warn!("评论自动审核调度暂时不可用，稍后重试"),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::status::ArticleStatus;
    use sea_orm::{ActiveModelTrait, Database, IntoActiveModel, Set};

    /// 只在独立的临时 PostgreSQL 中验证过期审核、人工优先和重复写回。
    #[tokio::test]
    #[ignore = "需要已迁移的 OXIDE_MANAGEMENT_TEST_DATABASE_URL，库名必须为 oxide_management_test"]
    async fn stale_agent_cannot_override_manual_or_resubmitted_review() {
        let url = std::env::var("OXIDE_MANAGEMENT_TEST_DATABASE_URL").unwrap();
        assert_eq!(
            url::Url::parse(&url).unwrap().path(),
            "/oxide_management_test"
        );
        let db = Database::connect(url).await.unwrap();
        let now = Utc::now();
        let article = article::ActiveModel {
            slug: Set(uuid::Uuid::new_v4().to_string()),
            title: Set("审核并发测试".into()),
            document: Set(json!({"type":"markdown","source":"test"})),
            rendered_html: Set("test".into()),
            status: Set(ArticleStatus::Draft),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let row = comment::ActiveModel {
            article_id: Set(article.id),
            nickname: Set("测试".into()),
            email_hash: Set("test".into()),
            body: Set("测试正文".into()),
            status: Set(CommentStatus::Pending),
            created_at: Set(now),
            review_version: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let mut manual = row.clone().into_active_model();
        manual.status = Set(CommentStatus::Rejected);
        manual.review_source = Set(Some("manual".into()));
        manual.reviewed_at = Set(Some(now));
        manual.review_version = Set(1);
        let updated = manual.update(&db).await.unwrap();
        assert!(
            !finish(
                &db,
                row.id,
                0,
                CommentStatus::Approved,
                json!({"decision":"approved"})
            )
            .await
            .unwrap()
        );
        let mut requeued = updated.into_active_model();
        requeued.status = Set(CommentStatus::Pending);
        requeued.review_source = Set(None);
        requeued.reviewed_at = Set(None);
        requeued.review_version = Set(2);
        requeued.update(&db).await.unwrap();
        assert!(
            !finish(
                &db,
                row.id,
                0,
                CommentStatus::Approved,
                json!({"decision":"approved"})
            )
            .await
            .unwrap()
        );
        assert!(
            finish(
                &db,
                row.id,
                2,
                CommentStatus::Approved,
                json!({"decision":"approved"})
            )
            .await
            .unwrap()
        );
        assert!(
            !finish(
                &db,
                row.id,
                2,
                CommentStatus::Rejected,
                json!({"decision":"rejected"})
            )
            .await
            .unwrap()
        );
        comment::Entity::delete_by_id(row.id)
            .exec(&db)
            .await
            .unwrap();
        article::Entity::delete_by_id(article.id)
            .exec(&db)
            .await
            .unwrap();
    }
}
