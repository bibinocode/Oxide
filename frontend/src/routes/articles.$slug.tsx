import { Link, createFileRoute } from "@tanstack/react-router";
import { ArrowLeft, BookOpen, LockKeyhole } from "lucide-react";
import { Comments } from "../features/comment/components/Comments";
import { ArticlePresentation } from "../features/article/components/ArticlePresentation";
import { ArticleOutline } from "../features/article/components/ArticleOutline";
import { getArticleData } from "../lib/api/server";
import { ArticlePending } from "../features/article/components/ArticlePending";

export const Route = createFileRoute("/articles/$slug")({
  loader: ({ params }) => getArticleData({ data: params.slug }),
  pendingComponent: ArticlePending,
  pendingMs: 120,
  pendingMinMs: 0,
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
      <ArticleOutline
        bodyId="article-body"
        html={article.rendered_html}
        title={article.title}
        outline={article.outline}
        columnSlug={article.column_slug}
      />
      <main className="reading-width pt-10 md:pt-14">
        <Link
          to="/archive"
          search={{ page: 1 }}
          className="inline-flex items-center gap-2 font-mono text-xs text-muted hover:text-ink"
        >
          <ArrowLeft size={14} /> 写作归档
        </Link>
        <div className={`mt-9${article.locked ? " article-preview-locked" : ""}`}>
          <ArticlePresentation
            title={article.title}
            summary={article.summary}
            publishedAt={article.published_at}
            html={article.rendered_html}
            coverUrl={article.cover_url}
            slug={article.slug}
            bodyId="article-body"
          />
          {article.locked && article.column_slug && (
            <section className="article-paywall" aria-labelledby="article-paywall-heading">
              <div className="article-paywall-heading">
                <LockKeyhole size={17} aria-hidden="true" />
                <h2 id="article-paywall-heading">订阅后解锁剩余内容</h2>
              </div>
              <p>已免费试看前 30%，订阅小册即可阅读全文。</p>
              <Link
                to="/columns/$slug"
                params={{ slug: article.column_slug }}
                className="button-primary"
              >
                <BookOpen size={15} aria-hidden="true" /> 订阅小册
              </Link>
            </section>
          )}
        </div>
        <Comments slug={article.slug} initialComments={comments} />
      </main>
    </>
  );
}
