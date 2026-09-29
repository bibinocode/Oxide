//! 首版数据库表与索引。

use crate::v1_entity::{
    admin_user, article, article_category, article_revision, article_tag, asset, category, comment,
    search_job, site_setting, storage_provider, tag,
};
use sea_orm_migration::{
    prelude::*,
    sea_orm::{DbBackend, EntityName, EntityTrait, Schema},
};

/// 创建首版博客模型的迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 按外键依赖顺序建表，并创建实体声明的索引。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(DbBackend::Postgres);

        create_entity(manager, &schema, admin_user::Entity).await?;
        create_entity(manager, &schema, storage_provider::Entity).await?;
        create_entity(manager, &schema, asset::Entity).await?;
        create_entity(manager, &schema, article::Entity).await?;
        create_entity(manager, &schema, article_revision::Entity).await?;
        create_entity(manager, &schema, category::Entity).await?;
        create_entity(manager, &schema, tag::Entity).await?;
        create_entity(manager, &schema, article_category::Entity).await?;
        create_entity(manager, &schema, article_tag::Entity).await?;
        create_entity(manager, &schema, comment::Entity).await?;
        create_entity(manager, &schema, site_setting::Entity).await?;
        create_entity(manager, &schema, search_job::Entity).await?;

        Ok(())
    }

    /// 按依赖的逆序撤销首版表结构。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            search_job::Entity.table_name(),
            site_setting::Entity.table_name(),
            comment::Entity.table_name(),
            article_tag::Entity.table_name(),
            article_category::Entity.table_name(),
            tag::Entity.table_name(),
            category::Entity.table_name(),
            article_revision::Entity.table_name(),
            article::Entity.table_name(),
            asset::Entity.table_name(),
            storage_provider::Entity.table_name(),
            admin_user::Entity.table_name(),
        ] {
            manager
                .drop_table(Table::drop().table(table).to_owned())
                .await?;
        }

        Ok(())
    }
}

/// 从实体生成表、外键和索引，保持数据库结构与模型一致。
async fn create_entity<E>(
    manager: &SchemaManager<'_>,
    schema: &Schema,
    entity: E,
) -> Result<(), DbErr>
where
    E: EntityTrait + Copy,
{
    manager
        .create_table(schema.create_table_from_entity(entity))
        .await?;

    for index in schema.create_index_from_entity(entity) {
        manager.create_index(index).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 检查关键关联和索引由 SeaORM 实体生成，而非只停留在 Rust 类型中。
    #[test]
    fn generated_postgres_schema_has_required_relations() {
        let schema = Schema::new(DbBackend::Postgres);

        let comments = schema.create_table_from_entity(comment::Entity);
        assert_eq!(comments.get_foreign_key_create_stmts().len(), 2);

        let article_categories = schema.create_table_from_entity(article_category::Entity);
        assert_eq!(article_categories.get_foreign_key_create_stmts().len(), 2);
        assert_eq!(article_categories.get_columns().len(), 2);

        let assets = schema.create_table_from_entity(asset::Entity);
        assert_eq!(assets.get_foreign_key_create_stmts().len(), 1);
        assert!(schema.create_index_from_entity(asset::Entity).len() >= 2);

        let search_jobs = schema.create_table_from_entity(search_job::Entity);
        assert!(search_jobs.get_foreign_key_create_stmts().is_empty());
    }
}
