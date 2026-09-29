export interface ArticleSummary {
  public_id: string;
  slug: string;
  title: string;
  summary: string | null;
  published_at: string | null;
}

export interface ArticleDetail extends ArticleSummary {
  rendered_html: string;
  cover_url: string | null;
}

export interface ArticlePage {
  items: ArticleSummary[];
  page: number;
  per_page: number;
  total: number;
}

export interface SiteSettings {
  site_name: string;
  description: string | null;
  base_url: string;
  presentation: SitePresentation;
}

export interface SiteItem {
  title: string;
  url: string;
  description: string;
  avatar_url?: string;
  stat_text?: string;
}
export interface SiteSection {
  enabled: boolean;
  items: SiteItem[];
}
export interface SitePresentation {
  home_intro: HomeIntroduction;
  footer_text: string;
  contacts: SiteSection;
  projects: SiteSection;
  services: SiteSection;
}

export interface HomeIntroduction {
  enabled: boolean;
  body: string;
  portrait_url: string;
  portrait_alt: string;
  xiaohongshu?: XiaohongshuCard;
}

export interface XiaohongshuCard {
  url: string;
  name: string;
  handle: string;
  bio: string;
  followers: string;
  likes: string;
}

export function emptyXiaohongshuCard(): XiaohongshuCard {
  return { url: "", name: "", handle: "", bio: "", followers: "", likes: "" };
}

/** 旧站点未填写模块时保持空列表，不显示占位或虚构内容。 */
export function emptyPresentation(): SitePresentation {
  return {
    home_intro: {
      enabled: false,
      body: "",
      portrait_url: "",
      portrait_alt: "",
      xiaohongshu: emptyXiaohongshuCard(),
    },
    footer_text: "",
    contacts: { enabled: false, items: [] },
    projects: { enabled: false, items: [] },
    services: { enabled: false, items: [] },
  };
}

export interface Taxonomy {
  name: string;
  slug: string;
}

export interface Comment {
  public_id: string;
  parent_public_id: string | null;
  nickname: string;
  body: string;
  avatar_url: string;
  created_at: string;
}

export interface AdminArticle extends ArticleSummary {
  status: "draft" | "published";
  document: Record<string, unknown>;
  updated_at: string;
  notion_page_id: string | null;
  notion_last_edited_at: string | null;
}

export interface NotionPage {
  id: string;
  title: string;
  url: string;
  last_edited_at: string;
  article_public_id: string | null;
  article_status: "draft" | "published" | null;
  synced_edited_at: string | null;
  local_changes: boolean;
}

export interface NotionPageList {
  items: NotionPage[];
  next_cursor: string | null;
}

export interface NotionSyncResult {
  outcome: "created" | "updated" | "unchanged";
  article: AdminArticle;
  warnings: string[];
}

export interface Session {
  username: string;
  csrf_token: string;
}

export interface ApiError {
  code: string;
  message: string;
}
