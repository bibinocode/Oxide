import { useEffect, useRef } from "react";
import { Search } from "lucide-react";
import { Link, createFileRoute } from "@tanstack/react-router";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { getArchiveData, getSearchResults } from "../lib/api/server";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { RevealContent } from "../components/layout/RevealContent";
import { groupByDate } from "../lib/utils/dateGroups";

export const Route = createFileRoute("/archive")({
  validateSearch: (
    search: Record<string, unknown>,
  ): { page: number; q?: string; focus?: boolean } => ({
    q: typeof search.q === "string" ? search.q.trim() || undefined : undefined,
    focus: search.focus === true || search.focus === "true" ? true : undefined,
    page: Number(search.page) > 0 ? Math.floor(Number(search.page)) : 1,
  }),
  loaderDeps: ({ search }) => ({ page: search.page, q: search.q }),
  loader: async ({ deps }) => {
    const [archive, results] = await Promise.all([
      getArchiveData({ data: deps.q ? 1 : deps.page }),
      deps.q
        ? getSearchResults({ data: { query: deps.q, page: deps.page } })
        : Promise.resolve(null),
    ]);
    return { ...archive, articles: results ?? archive.articles };
  },
  pendingComponent: ContentSkeleton,
  component: Archive,
});

/** 年份是独立的时间归档维度，未分类文章也按发布日期自然归入对应年份。 */
function Archive() {
  const { articles: page, categories, tags } = Route.useLoaderData();
  const { q, focus } = Route.useSearch();
  const navigate = Route.useNavigate();
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (focus) input.current?.focus();
  }, [focus]);
  const years = groupByDate(page.items, (article) => article.published_at, "year");
  return (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="写作归档" detail={`共 ${page.total} 篇文章`} />
      <form
        className="mt-7 flex gap-2"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          const query = input.current?.value.trim() || undefined;
          void navigate({ search: { page: 1, q: query } });
        }}
      >
        <label className="sr-only" htmlFor="archive-search">
          搜索文章关键词
        </label>
        <input
          key={q ?? ""}
          ref={input}
          id="archive-search"
          name="q"
          className="field min-w-0"
          defaultValue={q ?? ""}
          placeholder="搜索文章…"
          type="search"
        />
        <button type="submit" className="button-primary" aria-label="搜索文章">
          <Search size={18} />
        </button>
      </form>
      {q && (
        <div className="mt-4 flex items-center justify-between gap-3 text-sm">
          <p role="status" className="text-muted">
            “{q}” · {page.total} 条结果
          </p>
          <Link to="/archive" search={{ page: 1 }} className="text-accent">
            清除搜索
          </Link>
        </div>
      )}
      {(categories.length > 0 || tags.length > 0) && (
        <nav className="archive-taxonomy mt-7" aria-label="写作分类与标签">
          {categories.map((item) => (
            <Link
              key={item.slug}
              to="/categories/$slug"
              params={{ slug: item.slug }}
              search={{ page: 1 }}
            >
              {item.name}
            </Link>
          ))}
          {tags.map((item) => (
            <Link
              key={item.slug}
              to="/tags/$slug"
              params={{ slug: item.slug }}
              search={{ page: 1 }}
            >
              #{item.name}
            </Link>
          ))}
        </nav>
      )}
      <div className="mt-8 space-y-8">
        {q ? (
          <RevealContent>
            {page.items.map((article) => (
              <ArticleRow key={article.public_id} article={article} />
            ))}
          </RevealContent>
        ) : (
          years.map((year) => (
            <section
              key={year.key}
              className="archive-year"
              aria-labelledby={`archive-${year.key}`}
            >
              {year.key !== "undated" && (
                <span className="archive-year-folio" aria-hidden="true">
                  {year.key.slice(-2)}
                </span>
              )}
              <h2
                id={`archive-${year.key}`}
                className="text-sm font-medium tabular-nums text-muted"
              >
                {year.label}
              </h2>
              <RevealContent className="mt-2">
                {year.items.map((article) => (
                  <ArticleRow key={article.public_id} article={article} />
                ))}
              </RevealContent>
            </section>
          ))
        )}
      </div>
      {page.items.length === 0 && (
        <p className="border-t border-line py-12 text-muted">
          {q ? "没有找到相关文章。" : "暂无文章。"}
        </p>
      )}
      <nav className="mt-8 flex items-center gap-4 font-mono text-xs" aria-label="归档分页">
        {page.page > 1 && (
          <Link to="/archive" search={{ page: page.page - 1, q }} className="text-accent">
            上一页
          </Link>
        )}
        <span className="text-muted">第 {page.page} 页</span>
        {page.page * page.per_page < page.total && (
          <Link to="/archive" search={{ page: page.page + 1, q }} className="text-accent">
            下一页
          </Link>
        )}
      </nav>
    </main>
  );
}
