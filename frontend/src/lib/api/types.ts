export interface ArticleSummary {
  public_id: string;
  slug: string;
  title: string;
  summary: string | null;
  cover_url: string | null;
  published_at: string | null;
  paid_column_public_id: string | null;
  subscriber_only: boolean;
}

export interface ArticleDetail extends ArticleSummary {
  outline: { level: 2 | 3; label: string; available: boolean }[];
  rendered_html: string;
  cover_url: string | null;
  locked: boolean;
  column_slug: string | null;
}

export interface ColumnSummary {
  visible: boolean;
  public_id: string;
  slug: string;
  title: string;
  description: string;
  price_cents: number;
}

export interface ColumnDetail extends ColumnSummary {
  articles: ArticleSummary[];
  subscribed: boolean;
  payment_available: boolean;
}

export interface ArticleAccess {
  column_public_id: string | null;
  subscriber_only: boolean;
}

export interface ReaderSession {
  username: string;
  csrf_token: string;
}

export interface CheckoutOrder {
  order_id: string;
  code_url: string;
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
  about_body: string;
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
    about_body: "",
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
  visible: boolean;
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
  access: ArticleAccess;
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

/** 标准技能包的发现元数据，不包含指令与附属文件正文。 */
export interface SkillSummary {
  name: string;
  description: string;
  enabled: boolean;
  manual_only: boolean;
  license: string | null;
  compatibility: string | null;
  warnings: string[];
}

export interface SkillCatalog {
  skills: SkillSummary[];
  warnings: string[];
}

/** 原始 SKILL.md 与全部配套文件的轻量目录。 */
export interface SkillDetail {
  skill: SkillSummary;
  document: string;
  files: { path: string; size: number }[];
  source: string | null;
}
/** 网络搜索的非敏感配置；密钥仅由服务器环境提供。 */
export interface WebSearchSettings {
  enabled: boolean;
  tasks: string[];
  search_engine: "search_std" | "search_pro" | "search_pro_sogou" | "search_pro_quark";
  count: number;
  search_recency_filter: "oneDay" | "oneWeek" | "oneMonth" | "oneYear" | "noLimit";
  content_size: "medium" | "high";
  search_domain_filter: string | null;
}
export interface AgentToolDescriptor {
  name: string;
  title: string;
  description: string;
  configured: boolean;
  settings: WebSearchSettings;
  input_schema: unknown;
}
export interface WebSearchResponse {
  query: string;
  request_id: string;
  results: {
    title: string;
    content: string;
    url: string;
    source: string;
    reference: string;
    publish_date: string | null;
    truncated: boolean;
  }[];
}
