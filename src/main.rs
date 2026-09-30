//! 博客 API 进程入口。

use std::sync::Arc;

use anyhow::{Context, Result};
use rust_oxide::{
    config::Config,
    infrastructure::article_repository::SeaOrmArticleRepository,
    infrastructure::search::{SearchEngine, rebuild, run_worker},
    web::{self, AppState},
};
use sea_orm::{ConnectOptions, Database};
use tracing_subscriber::EnvFilter;

/// 初始化配置、日志和连接池，然后运行 Axum。
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    // 初始化 tracing 日志记录器
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&config.log_filter).context("RUST_LOG 格式无效")?)
        .try_init()
        .map_err(|error| anyhow::anyhow!("初始化 tracing 失败: {error}"))?;

    // 初始化数据库连接池
    let mut options = ConnectOptions::new(config.database_url.clone());
    options.max_connections(config.database_max_connections);
    let db = Database::connect(options)
        .await
        .context("连接 PostgreSQL 失败")?;
    let redis = redis::Client::open(config.redis_url.as_str()).context("Redis 地址无效")?;
    rust_oxide::web::admin::auth::bootstrap_admin(
        &db,
        config.admin_username.as_deref(),
        config.admin_password.as_deref(),
    )
    .await?;

    // 初始化搜索索引
    let search = Arc::new(SearchEngine::open(std::path::Path::new(
        &config.search_index_dir,
    ))?);
    // 处理命令行参数
    if let Some(argument) = std::env::args().nth(1) {
        if argument != "--reindex" {
            anyhow::bail!("未知命令参数: {argument}");
        }
        let count = rebuild(&db, search).await?;
        tracing::info!(articles = count, "搜索索引重建完成");
        return Ok(());
    }
    // 启动搜索索引重建线程
    tokio::spawn(run_worker(db.clone(), search.clone()));
    // 初始化应用状态
    let state: AppState = AppState {
        tools: Arc::new(rust_oxide::agent::tools::ToolRegistry::new(
            std::path::PathBuf::from(config.agent_tools_config),
            config.web_search_api_key,
        )?),
        skills: Arc::new(rust_oxide::infrastructure::agent_skills::SkillStore::new(
            std::path::PathBuf::from(config.agent_skills_dir),
        )),
        articles: Arc::new(SeaOrmArticleRepository::new(db.clone())),
        db,
        redis,
        comment_hash_key: Arc::from(config.comment_hash_key),
        session_secure: config.session_secure,
        public_base_url: Arc::from(config.public_base_url),
        search: search.clone(),
        notion_api_key: config.notion_api_key.map(Arc::from),
    };

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .context("绑定 HTTP 地址失败")?;
    tracing::info!(address = %listener.local_addr()?, "博客 API 已启动");
    axum::serve(listener, web::router(state))
        .await
        .context("HTTP 服务异常退出")
}
