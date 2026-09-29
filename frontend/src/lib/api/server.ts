import { createServerFn } from "@tanstack/react-start";
import { serverApi } from "./client";
import type { ArticleDetail, ArticlePage, Comment, SiteSettings, Taxonomy } from "./types";

/** 根布局读取公开配置，管理员路由由自身表单读取。 */
export const getSiteData = createServerFn({ method: "GET" }).handler(() =>
  serverApi<SiteSettings>("/api/v1/site"),
);

export const getHomeData = createServerFn({ method: "GET" }).handler(async () => {
  const [site, articles, categories, tags] = await Promise.all([
    serverApi<SiteSettings>("/api/v1/site"),
    serverApi<ArticlePage>("/api/v1/articles"),
    serverApi<Taxonomy[]>("/api/v1/categories"),
    serverApi<Taxonomy[]>("/api/v1/tags"),
  ]);
  return { site, articles, categories, tags };
});

export const getArticleData = createServerFn({ method: "GET" })
  .validator((slug: string) => slug)
  .handler(async ({ data: slug }) => {
    const [article, comments] = await Promise.all([
      serverApi<ArticleDetail>(`/api/v1/articles/${encodeURIComponent(slug)}`),
      serverApi<Comment[]>(`/api/v1/articles/${encodeURIComponent(slug)}/comments`),
    ]);
    return { article, comments };
  });

export const getArticlePage = createServerFn({ method: "GET" })
  .validator((page: number) => page)
  .handler(({ data: page }) => serverApi<ArticlePage>(`/api/v1/articles?page=${page}&per_page=20`));

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
