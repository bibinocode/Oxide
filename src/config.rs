//! 进程环境配置及启动前校验。

use std::{env, net::SocketAddr};

use anyhow::{Context, Result, bail};

/// 后端进程所需的连接和监听配置。
#[derive(Clone)]
pub struct Config {
    /// PostgreSQL 连接地址，不允许写入日志。
    pub database_url: String,
    /// Redis 连接地址，不允许写入日志。
    pub redis_url: String,
    /// Axum 监听地址。
    pub bind: SocketAddr,
    /// SeaORM 连接池上限。
    pub database_max_connections: u32,
    /// tracing 的过滤规则。
    pub log_filter: String,
    /// 评论邮箱的 HMAC 密钥，仅在服务端使用。
    pub comment_hash_key: String,
    /// 可选的首次启动管理员名称。
    pub admin_username: Option<String>,
    /// 可选的首次启动管理员密码，不写入日志。
    pub admin_password: Option<String>,
    /// HTTPS 部署时为会话 Cookie 添加 Secure 标记。
    pub session_secure: bool,
    /// 站点设置尚未创建时用于 RSS 绝对链接的回退地址。
    pub public_base_url: String,
    /// Tantivy 持久化索引目录。
    pub search_index_dir: String,
    /// 可选的 Notion 集成密钥，只传入后端服务。
    pub notion_api_key: Option<String>,
}

impl Config {
    /// 先读取可选的 `.env`，再从进程环境加载配置；进程环境优先。
    pub fn load() -> Result<Self> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("读取 .env 失败"),
        }

        Self::from_values(|key| env::var(key).ok())
    }

    /// 将配置解析与环境访问分离，便于测试缺失值和非法值。
    fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let database_url = required(&get, "DATABASE_URL")?;
        let redis_url = required(&get, "REDIS_URL")?;
        let comment_hash_key = required(&get, "COMMENT_HASH_KEY")?;
        if comment_hash_key.len() < 32 {
            bail!("COMMENT_HASH_KEY 至少需要 32 字节");
        }
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            bail!("DATABASE_URL 必须使用 PostgreSQL 地址");
        }
        if !redis_url.starts_with("redis://") && !redis_url.starts_with("rediss://") {
            bail!("REDIS_URL 必须使用 Redis 地址");
        }

        let bind = get("APP_BIND")
            .unwrap_or_else(|| "127.0.0.1:3001".into())
            .parse()
            .context("APP_BIND 必须是 IP:端口")?;
        let database_max_connections = get("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|| "10".into())
            .parse::<u32>()
            .context("DATABASE_MAX_CONNECTIONS 必须是正整数")?;
        if database_max_connections == 0 {
            bail!("DATABASE_MAX_CONNECTIONS 必须大于 0");
        }
        let admin_username = get("ADMIN_USERNAME").filter(|value| !value.trim().is_empty());
        let admin_password = get("ADMIN_PASSWORD").filter(|value| !value.is_empty());
        if admin_username.is_some() != admin_password.is_some() {
            bail!("ADMIN_USERNAME 与 ADMIN_PASSWORD 必须同时设置");
        }
        if admin_password
            .as_ref()
            .is_some_and(|password| password.len() < 12)
        {
            bail!("ADMIN_PASSWORD 至少需要 12 字符");
        }
        let session_secure = get("SESSION_SECURE")
            .unwrap_or_else(|| "false".into())
            .parse()
            .context("SESSION_SECURE 必须为 true 或 false")?;
        let public_base_url =
            get("PUBLIC_BASE_URL").unwrap_or_else(|| "http://127.0.0.1:3000".into());
        let parsed = url::Url::parse(&public_base_url).context("PUBLIC_BASE_URL 格式无效")?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            bail!("PUBLIC_BASE_URL 必须是 HTTP(S) 绝对地址");
        }

        Ok(Self {
            database_url,
            redis_url,
            bind,
            database_max_connections,
            log_filter: get("RUST_LOG").unwrap_or_else(|| "rust_oxide=info,tower_http=info".into()),
            comment_hash_key,
            admin_username,
            admin_password,
            session_secure,
            public_base_url: public_base_url.trim_end_matches('/').into(),
            search_index_dir: get("SEARCH_INDEX_DIR")
                .unwrap_or_else(|| "./data/search-index".into()),
            notion_api_key: get("NOTION_API_KEY").filter(|value| !value.trim().is_empty()),
        })
    }
}

/// 必填配置不允许缺失或仅包含空白字符。
fn required(get: &impl Fn(&str) -> Option<String>, key: &str) -> Result<String> {
    let value = get(key).context(format!("缺少环境变量 {key}"))?;
    if value.trim().is_empty() {
        bail!("环境变量 {key} 不能为空");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 使用固定输入验证默认值，不修改全局环境变量。
    fn parse(values: &[(&str, &str)]) -> Result<Config> {
        Config::from_values(|key| {
            if key == "COMMENT_HASH_KEY" {
                return Some("test-secret-with-at-least-32-bytes".into());
            }
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).into())
        })
    }

    #[test]
    fn loads_defaults_and_required_urls() {
        let config = parse(&[
            ("DATABASE_URL", "postgres://localhost/db"),
            ("REDIS_URL", "redis://localhost/"),
        ])
        .unwrap();
        assert_eq!(config.bind.to_string(), "127.0.0.1:3001");
        assert_eq!(config.database_max_connections, 10);
    }

    #[test]
    fn rejects_missing_or_invalid_values() {
        assert!(parse(&[]).is_err());
        assert!(
            parse(&[
                ("DATABASE_URL", "sqlite::memory:"),
                ("REDIS_URL", "redis://localhost/")
            ])
            .is_err()
        );
        assert!(
            parse(&[
                ("DATABASE_URL", "postgres://localhost/db"),
                ("REDIS_URL", "redis://localhost/"),
                ("DATABASE_MAX_CONNECTIONS", "0")
            ])
            .is_err()
        );
    }
}
