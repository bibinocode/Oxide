//! Agent 工具的配置与权限边界；密钥不属于可序列化配置。

use super::agent::AgentTask;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 管理员选择搜索引擎，模型不能自行切换计费方式。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
pub enum SearchEngine {
    #[default]
    #[serde(rename = "search_std")]
    Standard,
    #[serde(rename = "search_pro")]
    Pro,
    #[serde(rename = "search_pro_sogou")]
    Sogou,
    #[serde(rename = "search_pro_quark")]
    Quark,
}

/// 上游支持的时间范围，模型可以按具体问题缩小范围。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SearchRecency {
    OneDay,
    OneWeek,
    OneMonth,
    OneYear,
    #[default]
    NoLimit,
}

/// 搜索摘要详细程度；实际注入上下文仍受服务端字符预算限制。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchContentSize {
    #[default]
    Medium,
    High,
}

/// 非敏感工具设置，保存在服务端独立配置文件，不接收或返回 API Key。
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WebSearchSettings {
    pub enabled: bool,
    /// 写作和摘要的工具权限分别配置，缺省仅开放写作。
    pub tasks: Vec<String>,
    pub search_engine: SearchEngine,
    /// 宿主为上下文和调用成本设 1–10 条上限；搜狗仅允许 10。
    pub count: u8,
    pub search_recency_filter: SearchRecency,
    pub content_size: SearchContentSize,
    /// 管理员固定域名后，模型不能扩大这个范围。
    pub search_domain_filter: Option<String>,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            tasks: vec!["writing".into()],
            search_engine: SearchEngine::Standard,
            count: 5,
            search_recency_filter: SearchRecency::NoLimit,
            content_size: SearchContentSize::Medium,
            search_domain_filter: None,
        }
    }
}

impl WebSearchSettings {
    /// 验证引擎差异、任务权限和域名，防止保存上游不支持的组合。
    pub fn validated(mut self) -> Result<Self> {
        if self.tasks.is_empty()
            || self.tasks.len() > 2
            || self
                .tasks
                .iter()
                .any(|task| !matches!(task.as_str(), "writing" | "summary"))
        {
            bail!("至少选择写作或摘要任务，暂不支持其他任务");
        }
        self.tasks.sort();
        self.tasks.dedup();
        if !(1..=10).contains(&self.count) {
            bail!("每次搜索返回条数须在 1–10 之间");
        }
        if self.search_engine == SearchEngine::Sogou && self.count != 10 {
            bail!("搜狗引擎在当前宿主预算内仅支持 10 条结果");
        }
        self.search_domain_filter = normalize_domain(self.search_domain_filter.as_deref())?;
        if self.search_engine == SearchEngine::Quark && self.search_domain_filter.is_some() {
            bail!("夸克引擎不支持域名筛选");
        }
        Ok(self)
    }
    /// 仅向明确授权的任务注册工具；包中的 allowed-tools 不改变此权限。
    pub fn allows(&self, task: AgentTask) -> bool {
        self.enabled && self.tasks.iter().any(|value| value == task.as_str())
    }
}

/// 只接受裸 DNS 域名，不允许 URL、凭据、端口或路径。
pub fn normalize_domain(input: Option<&str>) -> Result<Option<String>> {
    let Some(domain) = input.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if domain.len() > 253 || domain.contains(['/', '\\', ':', '@', '?', '#', '%']) {
        bail!("域名筛选须填写裸域名，例如 www.example.com");
    }
    let url = url::Url::parse(&format!("https://{domain}"))?;
    let Some(url::Host::Domain(host)) = url.host() else {
        bail!("域名筛选不接受 IP 地址");
    };
    if !host.contains('.')
        || host.split('.').any(|part| {
            part.is_empty()
                || part.len() > 63
                || part.starts_with('-')
                || part.ends_with('-')
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        bail!("域名筛选格式无效");
    }
    Ok(Some(host.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_scope_engine_and_domain() {
        let settings = WebSearchSettings::default().validated().unwrap();
        assert!(settings.allows(AgentTask::Writing));
        assert!(!settings.allows(AgentTask::Summary));
        assert_eq!(
            normalize_domain(Some(" WWW.EXAMPLE.COM ")).unwrap(),
            Some("www.example.com".into())
        );
        for value in [
            "https://example.com",
            "localhost",
            "127.0.0.1",
            "example.com/path",
            "a@b.com",
            "example.com:80",
        ] {
            assert!(normalize_domain(Some(value)).is_err());
        }
        let mut bad = settings.clone();
        bad.count = 50;
        assert!(bad.validated().is_err());
        let mut bad = settings.clone();
        bad.search_engine = SearchEngine::Sogou;
        assert!(bad.validated().is_err());
        let mut bad = settings;
        bad.tasks = vec!["chat".into()];
        assert!(bad.validated().is_err());
    }
}
