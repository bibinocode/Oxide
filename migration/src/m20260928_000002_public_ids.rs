//! 为公开资源补充随机 UUID，不改动内部 bigint 关联。

use sea_orm_migration::prelude::*;
use uuid::Uuid;

/// 文章、素材和评论的外部标识迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 增加 UUID 列，分批回填旧记录，再添加非空与唯一约束。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in ["articles", "assets", "comments"] {
            add_public_id(manager, table).await?;
        }
        Ok(())
    }

    /// 撤销外部标识列及其唯一索引，保留内部主键和业务数据。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in ["comments", "assets", "articles"] {
            manager
                .drop_index(
                    Index::drop()
                        .name(format!("idx-{table}-public_id"))
                        .table(Alias::new(table))
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new(table))
                        .drop_column(Alias::new("public_id"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

/// 逐表回填，单批最多读取 500 个内部 ID，控制迁移内存占用。
async fn add_public_id(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .add_column(ColumnDef::new(Alias::new("public_id")).uuid())
                .to_owned(),
        )
        .await?;

    let connection = manager.get_connection();
    let mut last_id = None;
    loop {
        let mut select = Query::select();
        select
            .column(Alias::new("id"))
            .from(Alias::new(table))
            .order_by(Alias::new("id"), Order::Asc)
            .limit(500);
        if let Some(id) = last_id {
            select.and_where(Expr::col(Alias::new("id")).gt(id));
        }
        let rows = connection.query_all(&select).await?;
        if rows.is_empty() {
            break;
        }

        for row in rows {
            let id: i64 = row.try_get("", "id")?;
            connection
                .execute(
                    &Query::update()
                        .table(Alias::new(table))
                        .value(Alias::new("public_id"), Uuid::new_v4())
                        .and_where(Expr::col(Alias::new("id")).eq(id))
                        .to_owned(),
                )
                .await?;
            last_id = Some(id);
        }
    }

    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .modify_column(ColumnDef::new(Alias::new("public_id")).uuid().not_null())
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name(format!("idx-{table}-public_id"))
                .table(Alias::new(table))
                .col(Alias::new("public_id"))
                .unique()
                .to_owned(),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Migrator, v1_entity};
    use sea_orm_migration::sea_orm::entity::prelude::Json;
    use sea_orm_migration::sea_orm::{ActiveModelTrait, Database, DatabaseConnection, Set};

    /// 在专用空数据库中验证旧记录回填；正常单元测试不会接触开发数据库。
    #[tokio::test]
    #[ignore = "需要 MIGRATION_TEST_DATABASE_URL 指向专用空 PostgreSQL 数据库"]
    async fn backfills_existing_rows() {
        let url = std::env::var("MIGRATION_TEST_DATABASE_URL").expect("测试数据库地址未设置");
        let db = Database::connect(url).await.expect("连接测试数据库失败");
        Migrator::up(&db, Some(1)).await.expect("首版迁移失败");

        let now = sea_orm_migration::sea_orm::entity::prelude::DateTimeUtc::from_timestamp(0, 0)
            .expect("时间戳无效");
        let provider = v1_entity::storage_provider::ActiveModel {
            id: Set("legacy-provider".to_owned()),
            kind: Set(v1_entity::status::StorageProviderKind::AliyunOss),
            name: Set("旧存储".to_owned()),
            public_base_url: Set("https://example.invalid".to_owned()),
            upload_enabled: Set(true),
            created_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("旧提供商插入失败");

        let asset = v1_entity::asset::ActiveModel {
            id: Set(-5),
            provider_id: Set(provider.id),
            object_key: Set("legacy-image.png".to_owned()),
            mime_type: Set("image/png".to_owned()),
            size_bytes: Set(1),
            width: Set(Some(1)),
            height: Set(Some(1)),
            visibility: Set(v1_entity::status::AssetVisibility::Public),
            created_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("旧素材插入失败");

        let article = v1_entity::article::ActiveModel {
            id: Set(-7),
            slug: Set("legacy-article".to_owned()),
            title: Set("旧文章".to_owned()),
            summary: Set(None),
            document: Set(Json::Object(Default::default())),
            rendered_html: Set("<p>旧文章</p>".to_owned()),
            status: Set(v1_entity::status::ArticleStatus::Published),
            cover_asset_id: Set(Some(asset.id)),
            published_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("旧文章插入失败");

        let comment = v1_entity::comment::ActiveModel {
            id: Set(-9),
            article_id: Set(article.id),
            parent_id: Set(None),
            nickname: Set("访客".to_owned()),
            email_hash: Set("legacy-hash".to_owned()),
            body: Set("旧评论".to_owned()),
            status: Set(v1_entity::status::CommentStatus::Approved),
            created_at: Set(now),
            reviewed_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("旧评论插入失败");

        Migrator::up(&db, None).await.expect("外部 UUID 迁移失败");
        let article_public_id = read_public_id(&db, "articles", article.id).await;
        let asset_public_id = read_public_id(&db, "assets", asset.id).await;
        let comment_public_id = read_public_id(&db, "comments", comment.id).await;
        assert_eq!(article_public_id.get_version_num(), 4);
        assert_eq!(asset_public_id.get_version_num(), 4);
        assert_eq!(comment_public_id.get_version_num(), 4);
        assert_ne!(article_public_id, asset_public_id);
        assert_ne!(asset_public_id, comment_public_id);

        Migrator::up(&db, None).await.expect("迁移重复运行失败");
        assert_eq!(
            read_public_id(&db, "articles", article.id).await,
            article_public_id
        );
    }

    /// 通过结构化查询读取回填结果，不依赖新版业务实体。
    async fn read_public_id(db: &DatabaseConnection, table: &str, id: i64) -> Uuid {
        let row = db
            .query_one(
                &Query::select()
                    .column(Alias::new("public_id"))
                    .from(Alias::new(table))
                    .and_where(Expr::col(Alias::new("id")).eq(id))
                    .to_owned(),
            )
            .await
            .expect("读取外部 ID 失败")
            .expect("迁移后记录不存在");
        row.try_get("", "public_id").expect("外部 ID 类型错误")
    }
}
