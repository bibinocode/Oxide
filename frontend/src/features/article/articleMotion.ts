/** 列表与详情使用相同的合法 CSS 标识，让浏览器将缩略图连续变形为主图。 */
export function articleTransitionName(kind: "cover" | "title", slug: string): string {
  const encoded = Array.from(slug, (character) => character.codePointAt(0)!.toString(16)).join("-");
  return `article-${kind}-${encoded}`;
}

/** 只携带公开摘要，加载期间可显示已知信息，不额外请求全文或缓存私密内容。 */
export interface ArticleNavigationPreview {
  slug: string;
  title: string;
  cover_url: string | null;
}

declare module "@tanstack/history" {
  interface HistoryState {
    articlePreview?: ArticleNavigationPreview;
  }
}
