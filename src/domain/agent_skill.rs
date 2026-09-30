//! Agent Skills 文件格式：解析 YAML 元数据，保留原始 Markdown 和全部附属文件。

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// 核心文档大小上限，不限制作者的章节或目录结构。
pub const MAX_SKILL_BYTES: usize = 262_144;

/// 标准元数据与 Claude 扩展；未知字段原样保留，不冒充已支持的执行能力。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillManifest {
    /// 稳定技能名，也是安装目录名。
    pub name: String,
    /// 用途与触发场景，用于第一层技能发现。
    pub description: String,
    /// 作者声明的许可证。
    #[serde(default)]
    pub license: Option<String>,
    /// 作者声明的环境要求。
    #[serde(default)]
    pub compatibility: Option<String>,
    /// 作者声明的工具要求，不自动转化为服务端权限。
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<serde_yaml_ng::Value>,
    /// 为 true 时仅允许用户明确使用 /name 唤起。
    #[serde(default, rename = "disable-model-invocation")]
    pub manual_only: bool,
    /// 未实现扩展保留在包文件中，并向管理员提示兼容性边界。
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml_ng::Value>,
}

/// 列表只暴露元数据，不加载技能正文或附属资源进模型上下文。
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub manual_only: bool,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub warnings: Vec<String>,
}

/// 保留原文供编辑、导出和按需加载，不将包拆成数据库字段。
pub struct SkillDocument {
    pub manifest: SkillManifest,
    pub source: String,
    pub body: String,
}

/// 验证标准名称，同时排除 Windows 设备名和不安全目录名称。
pub fn valid_name(name: &str) -> bool {
    let reserved = matches!(name, "con" | "prn" | "aux" | "nul")
        || (name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && matches!(name.as_bytes()[3], b'1'..=b'9'));
    !reserved
        && !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// 按 YAML frontmatter 解析；支持多行描述、CRLF 与 UTF-8 BOM，不执行动态命令。
pub fn parse(source: &str) -> Result<SkillDocument> {
    if source.len() > MAX_SKILL_BYTES {
        bail!("SKILL.md 不能超过 256 KiB");
    }
    let normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let Some(rest) = normalized.strip_prefix("---\n") else {
        bail!("SKILL.md 必须以 YAML frontmatter 开始");
    };
    let Some(end) = rest
        .find("\n---\n")
        .or_else(|| rest.strip_suffix("\n---").map(str::len))
    else {
        bail!("缺少 YAML frontmatter 结束标记");
    };
    if end > 16_384 {
        bail!("Skill 元数据过长");
    }
    let manifest: SkillManifest = serde_yaml_ng::from_str(&rest[..end])
        .map_err(|_| anyhow::anyhow!("Skill YAML 无效，必须包含 name 和 description"))?;
    if !valid_name(&manifest.name) {
        bail!("name 必须为 1–64 位小写字母、数字或连字符，不能使用设备名、首尾或连续连字符");
    }
    if manifest.description.trim().is_empty() || manifest.description.chars().count() > 1024 {
        bail!("description 必须为 1–1024 字符");
    }
    if manifest
        .compatibility
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        bail!("compatibility 不能超过 500 字符");
    }
    let body = rest.get(end + 5..).unwrap_or_default().to_owned();
    Ok(SkillDocument {
        manifest,
        source: source.to_owned(),
        body,
    })
}

impl SkillManifest {
    /// 显示兼容性限制；第三方包原文不会因不支持扩展而被重写。
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.allowed_tools.is_some() {
            warnings.push(
                "allowed-tools 仅作为声明保留；当前只提供技能文件读取，不授予脚本或外部工具权限"
                    .into(),
            );
        }
        for field in ["context", "agent", "hooks", "model"] {
            if self.extra.contains_key(field) {
                warnings.push(format!("{field} 扩展暂不执行，已保留原配置"));
            }
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_metadata_and_preserves_source() {
        let source = "---\r\nname: pdf-tools\r\ndescription: >-\r\n  Read and merge PDF files.\r\n  Use for document tasks.\r\nmetadata:\r\n  author: example\r\nallowed-tools: Bash Read\r\n---\r\n# PDF\r\nSee references/forms.md\r\n";
        let document = parse(source).unwrap();
        assert_eq!(document.source, source);
        assert!(
            document
                .manifest
                .description
                .contains("Use for document tasks")
        );
        assert!(document.body.starts_with("# PDF"));
        assert!(document.manifest.extra.contains_key("metadata"));
        assert_eq!(document.manifest.warnings().len(), 1);
    }

    #[test]
    fn rejects_invalid_names_and_missing_metadata() {
        for name in ["../test", "Test", "con", "com1", "two--words", "-a", "a-"] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("a"));
        assert!(parse("# No metadata").is_err());
        assert!(parse("---\nname: test\ndescription: ''\n---\nbody").is_err());
        assert!(parse("---\nname: test\n---\nbody").is_err());
    }
}
