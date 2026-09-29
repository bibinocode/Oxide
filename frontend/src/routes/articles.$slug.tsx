import { Link, createFileRoute } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { Comments } from "../features/comment/components/Comments";
import { ArticlePresentation } from "../features/article/components/ArticlePresentation";
import { ArticleOutline } from "../features/article/components/ArticleOutline";
import { getArticleData } from "../lib/api/server";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";

export const Route = createFileRoute("/articles/$slug")({
  loader: ({ params }) => getArticleData({ data: params.slug }),
  pendingComponent: () => <ContentSkeleton article />,
  head: ({ loaderData }) => ({
    meta: [
      { title: loaderData ? `${loaderData.article.title} · Oxide` : "文章 · Oxide" },
      { name: "description", content: loaderData?.article.summary ?? "" },
    ],
  }),
  component: ArticlePage,
});

function ArticlePage() {
  const { article, comments } = Route.useLoaderData();
  return (
    <>
      <ArticleOutline bodyId="article-body" html={article.rendered_html} title={article.title} />
      <main className="reading-width pt-10 md:pt-14">
        <Link
          to="/archive"
          search={{ page: 1 }}
          className="inline-flex items-center gap-2 font-mono text-xs text-muted hover:text-ink"
        >
          <ArrowLeft size={14} /> 写作归档
        </Link>
        <div className="mt-9">
          <ArticlePresentation
            title={article.title}
            summary={article.summary}
            publishedAt={article.published_at}
            html={article.rendered_html}
            coverUrl={article.cover_url}
            bodyId="article-body"
          />
        </div>
        <Comments slug={article.slug} initialComments={comments} />
      </main>
    </>
  );
}
