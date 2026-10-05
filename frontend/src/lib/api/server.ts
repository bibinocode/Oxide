import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader, setResponseHeader } from "@tanstack/react-start/server";
import { serverApi } from "./client";
import type {
  ArticleDetail,
  ArticlePage,
  ColumnDetail,
  ColumnSummary,
  Comment,
  SiteSettings,
  Taxonomy,
} from "./types";

/** 每次 SSR 请求只转发当前浏览器的 Cookie，不建立跨读者缓存。 */
function readerRequest(): RequestInit {
  // 同时保护 SSR 页面和服务端函数响应，避免共享缓存混用不同读者的权益。
  setResponseHeader("Cache-Control", "private, no-store");
  const cookie = getRequestHeader("cookie");
  return { headers: cookie ? { cookie } : {}, cache: "no-store" };
}

/** 根布局读取公开配置，管理员路由由自身表单读取。 */
export const getSiteData = createServerFn({ method: "GET" }).handler(() =>
  serverApi<SiteSettings>("/api/v1/site"),
);

export const getHomeData = createServerFn({ method: "GET" }).handler(async () => {
  const [site, articles, columns] = await Promise.all([
    serverApi<SiteSettings>("/api/v1/site"),
    serverApi<ArticlePage>("/api/v1/articles"),
    serverApi<ColumnSummary[]>("/api/v1/columns"),
  ]);
  return { site, articles, columns };
});

export const getArticleData = createServerFn({ method: "GET" })
  .validator((slug: string) => slug)
  .handler(async ({ data: slug }) => {
    const [article, comments] = await Promise.all([
      serverApi<ArticleDetail>(`/api/v1/articles/${encodeURIComponent(slug)}`, readerRequest()),
      serverApi<Comment[]>(`/api/v1/articles/${encodeURIComponent(slug)}/comments`),
    ]);
    return { article, comments };
  });

/** 专栏详情的订阅状态也必须按当前读者会话读取。 */
export const getColumnData = createServerFn({ method: "GET" })
  .validator((slug: string) => slug)
  .handler(({ data: slug }) =>
    serverApi<ColumnDetail>(`/api/v1/columns/${encodeURIComponent(slug)}`, readerRequest()),
  );

/** 专栏目录只包含公开元信息，可用于站点导航。 */
export const getColumns = createServerFn({ method: "GET" }).handler(() =>
  serverApi<ColumnSummary[]>("/api/v1/columns"),
);

export const getArticlePage = createServerFn({ method: "GET" })
  .validator((page: number) => page)
  .handler(({ data: page }) => serverApi<ArticlePage>(`/api/v1/articles?page=${page}&per_page=20`));

/** 年份归档与分类筛选入口共用一页，首页只保留写作和小册。 */
export const getArchiveData = createServerFn({ method: "GET" })
  .validator((page: number) => page)
  .handler(async ({ data: page }) => {
    const [articles, categories, tags] = await Promise.all([
      serverApi<ArticlePage>(`/api/v1/articles?page=${page}&per_page=20`),
      serverApi<Taxonomy[]>("/api/v1/categories"),
      serverApi<Taxonomy[]>("/api/v1/tags"),
    ]);
    return { articles, categories, tags };
  });

export const getTaxonomyPage = createServerFn({ method: "GET" })
  .validator((input: { kind: "categories" | "tags"; slug: string; page: number }) => input)
  .handler(async ({ data }) => {
    const [items, articles] = await Promise.all([
      serverApi<Taxonomy[]>(`/api/v1/${data.kind}`),
      serverApi<ArticlePage>(
        `/api/v1/${data.kind}/${encodeURIComponent(data.slug)}/articles?page=${data.page}`,
      ),
    ]);
    return { label: items.find((item) => item.slug === data.slug)?.name ?? data.slug, articles };
  });

export const getSearchResults = createServerFn({ method: "GET" })
  .validator((input: { query: string; page: number }) => input)
  .handler(({ data }) =>
    serverApi<ArticlePage>(`/api/v1/search?q=${encodeURIComponent(data.query)}&page=${data.page}`),
  );
