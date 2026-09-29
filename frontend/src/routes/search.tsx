import { Link, createFileRoute } from "@tanstack/react-router";
import { Search as SearchIcon } from "lucide-react";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { getSearchResults } from "../lib/api/server";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { RevealContent } from "../components/layout/RevealContent";

export const Route = createFileRoute("/search")({
  validateSearch: (search: Record<string, unknown>) => ({
    q: typeof search.q === "string" ? search.q : "",
    page: Number(search.page) > 0 ? Math.floor(Number(search.page)) : 1,
  }),
  loaderDeps: ({ search }) => ({ q: search.q, page: search.page }),
  loader: ({ deps }) => getSearchResults({ data: { query: deps.q, page: deps.page } }),
  pendingComponent: ContentSkeleton,
  component: SearchPage,
});

function SearchPage() {
  const { q } = Route.useSearch();
  const results = Route.useLoaderData();
  return (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="搜索文章" />
      <form action="/search" method="get" className="mt-8 flex gap-2">
        <label className="sr-only" htmlFor="search-q">
          关键词
        </label>
        <input
          id="search-q"
          name="q"
          className="field"
          defaultValue={q}
          placeholder="输入中文或英文关键词"
        />
        <button type="submit" className="button-primary" aria-label="搜索">
          <SearchIcon size={18} />
        </button>
      </form>
      <p className="mt-6 text-sm text-muted">
        {q ? `“${q}” · ${results.total} 条结果` : "输入关键词开始搜索"}
      </p>
      <RevealContent className="mt-8">
        {results.items.map((article) => (
          <ArticleRow key={article.public_id} article={article} />
        ))}
        {q && results.items.length === 0 && (
          <p className="border-t border-line py-12 text-muted">没有找到相关文章。</p>
        )}
      </RevealContent>
      <nav className="mt-8 flex items-center gap-4 font-mono text-xs" aria-label="搜索分页">
        {results.page > 1 && (
          <Link to="/search" search={{ q, page: results.page - 1 }} className="text-accent">
            上一页
          </Link>
        )}
        <span className="text-muted">第 {results.page} 页</span>
        {results.page * results.per_page < results.total && (
          <Link to="/search" search={{ q, page: results.page + 1 }} className="text-accent">
            下一页
          </Link>
        )}
      </nav>
    </main>
  );
}
