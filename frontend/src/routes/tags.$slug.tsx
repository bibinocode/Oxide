import { Link, createFileRoute } from "@tanstack/react-router";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { getTaxonomyPage } from "../lib/api/server";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { RevealContent } from "../components/layout/RevealContent";

export const Route = createFileRoute("/tags/$slug")({
  validateSearch: (search: Record<string, unknown>) => ({
    page: Number(search.page) > 0 ? Math.floor(Number(search.page)) : 1,
  }),
  loaderDeps: ({ search }) => ({ page: search.page }),
  loader: ({ params, deps }) =>
    getTaxonomyPage({ data: { kind: "tags", slug: params.slug, page: deps.page } }),
  pendingComponent: ContentSkeleton,
  component: TagPage,
});

function TagPage() {
  const { slug } = Route.useParams();
  const { label, articles } = Route.useLoaderData();
  return (
    <main className="public-width pt-10 md:pt-14">
      <Link
        to="/archive"
        search={{ page: 1 }}
        className="mb-6 inline-flex items-center gap-2 text-sm text-muted hover:text-accent"
      >
        <span aria-hidden="true">←</span> 全部归档
      </Link>
      <PublicPageHeader title={`#${label}`} detail={`标签 · ${articles.total} 篇文章`} />
      <RevealContent className="mt-8">
        {articles.items.map((article) => (
          <ArticleRow article={article} key={article.public_id} />
        ))}
        {articles.items.length === 0 && (
          <p className="border-t border-line py-12 text-muted">暂无文章。</p>
        )}
      </RevealContent>
      <nav className="mt-8 flex items-center gap-4 font-mono text-xs" aria-label="标签分页">
        {articles.page > 1 && (
          <Link
            to="/tags/$slug"
            params={{ slug }}
            search={{ page: articles.page - 1 }}
            className="text-accent"
          >
            上一页
          </Link>
        )}
        <span className="text-muted">第 {articles.page} 页</span>
        {articles.page * articles.per_page < articles.total && (
          <Link
            to="/tags/$slug"
            params={{ slug }}
            search={{ page: articles.page + 1 }}
            className="text-accent"
          >
            下一页
          </Link>
        )}
      </nav>
    </main>
  );
}
