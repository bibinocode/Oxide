//! 公开站点模块配置；不包含模型密钥、存储凭据等私密字段。

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 联系方式、作品和服务共用的可排序展示项。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct SiteItem {
    /// 展示名称。
    pub title: String,
    /// 联系方式或外部项目地址。
    pub url: String,
    /// 项目或服务说明，联系方式可留空。
    #[serde(default)]
    pub description: String,
    /// 社交身份卡头像；留空时显示对应平台标识。
    #[serde(default)]
    pub avatar_url: String,
    /// 社交身份卡底部统计文字，例如订阅者数量。
    #[serde(default)]
    pub stat_text: String,
}

/// 可独立关闭的公开模块；关闭不会删除已填写内容。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default)]
pub struct SiteSection {
    /// 是否在公开页面展示。
    pub enabled: bool,
    /// 按数组顺序展示的内容。
    pub items: Vec<SiteItem>,
}

/// 站点公开展示设置，新增字段通过默认值兼容旧配置。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default)]
pub struct SitePresentation {
    /// 页脚署名；空值时使用站点名称。
    pub footer_text: String,
    /// 联系方式列表。
    pub contacts: SiteSection,
    /// 项目作品集。
    pub projects: SiteSection,
    /// 服务介绍。
    pub services: SiteSection,
}

impl SitePresentation {
    /// 校验公开文本和链接；只接受 HTTP(S)、mailto 和站内路径。
    pub fn is_valid(&self) -> bool {
        self.footer_text.chars().count() <= 200
            && [&self.contacts, &self.projects, &self.services]
                .iter()
                .all(|section| {
                    section.items.len() <= 30
                        && section.items.iter().all(|item| {
                            !item.title.trim().is_empty()
                                && item.title.chars().count() <= 80
                                && item.description.chars().count() <= 500
                                && item.stat_text.chars().count() <= 80
                                && (item.avatar_url.is_empty()
                                    || item.avatar_url.len() <= 2048
                                        && url::Url::parse(&item.avatar_url).is_ok_and(|url| {
                                            matches!(url.scheme(), "http" | "https")
                                                && url.host_str().is_some()
                                        }))
                                && item.url.len() <= 2048
                                && (item.url.starts_with('/')
                                    && !item.url.starts_with("//")
                                    && !item.url.contains('\\')
                                    || url::Url::parse(&item.url).is_ok_and(|url| {
                                        matches!(url.scheme(), "http" | "https")
                                            && url.host_str().is_some()
                                            || url.scheme() == "mailto" && !url.path().is_empty()
                                    }))
                        })
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_validate_links_even_when_hidden() {
        let mut config = SitePresentation::default();
        config.contacts.items.push(SiteItem {
            title: "邮箱".into(),
            url: "mailto:hello@example.com".into(),
            description: String::new(),
            avatar_url: String::new(),
            stat_text: String::new(),
        });
        assert!(config.is_valid());
        config.contacts.items[0].url = "javascript:alert(1)".into();
        assert!(!config.is_valid());
        config.contacts.items[0].url = "//untrusted.example".into();
        assert!(!config.is_valid());
        config.contacts.items[0].url = "/\\untrusted.example".into();
        assert!(!config.is_valid());
        config.contacts.items[0].url = "/projects".into();
        assert!(config.is_valid());
        config.contacts.items[0].avatar_url = "javascript:alert(1)".into();
        assert!(!config.is_valid());
        config.contacts.items[0].avatar_url = "https://example.com/avatar.jpg".into();
        assert!(config.is_valid());
        config.contacts.items[0].stat_text = "x".repeat(81);
        assert!(!config.is_valid());
    }
}
