//! 使用固定示例正文验证当前摘要模型，不读取或修改文章数据。

use anyhow::{Context, Result};
use rust_oxide::{agent::summary, config::Config, domain::agent::AgentTask, infrastructure::agent};
use sea_orm::Database;

/// 仅通过任务绑定读取模型配置并执行一次短摘要请求。
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let db = Database::connect(&config.database_url).await?;
    let provider = agent::text_provider(&db, AgentTask::Summary)
        .await?
        .context("没有可用的摘要文本模型")?;
    let prompt = summary::prompt(
        "Rust 所有权与博客服务",
        "Rust 的所有权规则让每个值在任意时刻只有一个所有者。借用允许函数读取或修改数据，同时在编译期避免悬垂引用。对博客服务而言，这能让请求处理与后台任务共享明确的资源边界。",
    )?;
    let output = agent::generate_text(
        &provider,
        &config.comment_hash_key,
        &rust_oxide::agent::context::AgentContext::skills_only(
            rust_oxide::agent::skills::SkillContext::discover(
                std::sync::Arc::new(rust_oxide::infrastructure::agent_skills::SkillStore::new(
                    std::path::PathBuf::from(config.agent_skills_dir),
                )),
                summary::SYSTEM_PROMPT,
                "",
            )
            .await?,
        ),
        prompt,
        summary::GENERATION_OPTIONS,
    )
    .await?;
    println!(
        "provider={} summary={}",
        provider.id,
        summary::validate_output(&output)?
    );
    Ok(())
}
