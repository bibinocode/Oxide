import { Link, createFileRoute } from "@tanstack/react-router";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { getArticlePage } from "../lib/api/server";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { RevealContent } from "../components/layout/RevealContent";

export const Route = createFileRoute("/archive")({
  validateSearch: (search: Record<string, unknown>) => ({
    page: Number(search.page) > 0 ? Math.floor(Number(search.page)) : 1,
  }),
  loaderDeps: ({ search }) => ({ page: search.page }),
  loader: ({ deps }) => getArticlePage({ data: deps.page }),
  pendingComponent: ContentSkeleton,
  component: Archive,
});

function Archive() {
  const page = Route.useLoaderData();
  return (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="写作归档" detail={`共 ${page.total} 篇文章`} />
      <RevealContent className="mt-8">
        {page.items.map((article) => (
          <ArticleRow key={article.public_id} article={article} />
        ))}
      </RevealContent>
      {page.items.length === 0 && (
        <p className="border-t border-line py-12 text-muted">暂无文章。</p>
      )}
      <nav className="mt-8 flex items-center gap-4 font-mono text-xs" aria-label="归档分页">
        {page.page > 1 && (
          <Link to="/archive" search={{ page: page.page - 1 }} className="text-accent">
            上一页
          </Link>
        )}
        <span className="text-muted">第 {page.page} 页</span>
        {page.page * page.per_page < page.total && (
          <Link to="/archive" search={{ page: page.page + 1 }} className="text-accent">
            下一页
          </Link>
        )}
      </nav>
    </main>
  );
}
