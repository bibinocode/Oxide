//! 从环境变量连接 PostgreSQL 并执行尚未应用的迁移。

use sea_orm_migration::{MigratorTrait, sea_orm::Database};

use rust_oxide_migration::Migrator;

/// 迁移程序入口；数据库地址仅从运行环境读取。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let connection = Database::connect(database_url).await?;
    Migrator::up(&connection, None).await?;
    println!("Database migrations applied.");
    Ok(())
}
