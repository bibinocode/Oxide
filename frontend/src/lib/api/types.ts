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
  footer_text: string;
  contacts: SiteSection;
  projects: SiteSection;
  services: SiteSection;
}

/** 旧站点未填写模块时保持空列表，不显示占位或虚构内容。 */
export function emptyPresentation(): SitePresentation {
  return {
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
}

export interface Session {
  username: string;
  csrf_token: string;
}

export interface ApiError {
  code: string;
  message: string;
}
