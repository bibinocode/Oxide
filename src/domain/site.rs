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

/// 首页个人介绍；照片指向本站已公开的素材，空值时只显示文字。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default)]
pub struct HomeIntroduction {
    /// 是否启用独立的个人介绍布局。
    pub enabled: bool,
    /// 按换行分段的介绍正文。
    pub body: String,
    /// 已公开肖像素材的稳定站内地址。
    pub portrait_url: String,
    /// 肖像替代文字。
    pub portrait_alt: String,
    /// 小红书公开资料卡；平台不提供稳定公开接口时使用站长维护的快照。
    pub xiaohongshu: XiaohongshuCard,
}

/// 与首页介绍中的小红书个人主页链接匹配的资料快照。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default)]
pub struct XiaohongshuCard {
    /// 个人主页地址；留空时禁用资料快照。
    pub url: String,
    /// 卡片姓名。
    pub name: String,
    /// 小红书号。
    pub handle: String,
    /// 个人简介，可包含换行。
    pub bio: String,
    /// 粉丝数展示文本，例如 10+。
    pub followers: String,
    /// 获赞与收藏展示文本，例如 1千+。
    pub likes: String,
}

impl XiaohongshuCard {
    /// 限定为小红书个人主页，不允许任意外链冒充资料卡。
    fn is_valid(&self) -> bool {
        if self.url.is_empty() {
            return self.name.is_empty()
                && self.handle.is_empty()
                && self.bio.is_empty()
                && self.followers.is_empty()
                && self.likes.is_empty();
        }
        let valid_url = url::Url::parse(&self.url).is_ok_and(|url| {
            url.scheme() == "https"
                && url
                    .host_str()
                    .is_some_and(|host| host == "xiaohongshu.com" || host == "www.xiaohongshu.com")
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
                && url.path().strip_prefix("/user/profile/").is_some_and(|id| {
                    id.len() == 24 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        });
        valid_url
            && self.url.len() <= 2048
            && self.name.chars().count() <= 80
            && self.handle.chars().count() <= 80
            && self.bio.chars().count() <= 500
            && self.followers.chars().count() <= 32
            && self.likes.chars().count() <= 32
    }
}

/// 站点公开展示设置，新增字段通过默认值兼容旧配置。
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default)]
pub struct SitePresentation {
    /// 首页个人介绍。
    pub home_intro: HomeIntroduction,
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
            && self.home_intro.body.chars().count() <= 2000
            && self.home_intro.portrait_alt.chars().count() <= 120
            && self.home_intro.xiaohongshu.is_valid()
            && (self.home_intro.portrait_url.is_empty()
                || self
                    .home_intro
                    .portrait_url
                    .strip_prefix("/media/")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()))
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

    #[test]
    fn home_portrait_requires_stable_media_url() {
        let mut config = SitePresentation::default();
        config.home_intro.portrait_url = "https://example.com/photo.jpg".into();
        assert!(!config.is_valid());
        config.home_intro.portrait_url = format!("/media/{}", uuid::Uuid::new_v4());
        assert!(config.is_valid());
        config.home_intro.body = "a".repeat(2001);
        assert!(!config.is_valid());
        let old: SitePresentation = serde_json::from_value(serde_json::json!({
            "footer_text": "旧站点",
            "contacts": { "enabled": false, "items": [] }
        }))
        .unwrap();
        assert!(!old.home_intro.enabled);
    }

    #[test]
    fn xiaohongshu_card_requires_matching_profile_url() {
        let mut config = SitePresentation::default();
        config.home_intro.xiaohongshu.url =
            "https://www.xiaohongshu.com/user/profile/5cbba503000000001101b6a2".into();
        config.home_intro.xiaohongshu.name = "站长".into();
        assert!(config.is_valid());
        config.home_intro.xiaohongshu.url =
            "https://xiaohongshu.com.evil.test/user/profile/5cbba503000000001101b6a2".into();
        assert!(!config.is_valid());
    }
}
