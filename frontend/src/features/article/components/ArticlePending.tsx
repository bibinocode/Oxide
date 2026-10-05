import { useRouterState } from "@tanstack/react-router";
import { ContentSkeleton } from "../../../components/layout/ContentSkeleton";
import { ArticleCover } from "./ArticleCover";
import { articleTransitionName } from "../articleMotion";

/** 从列表进入时保留真实主图与标题，只有未知正文使用加载占位。 */
export function ArticlePending() {
  const preview = useRouterState({
    select: (state) => {
      const value = state.location.state.articlePreview;
      return value && state.location.pathname === `/articles/${encodeURIComponent(value.slug)}`
        ? value
        : null;
    },
  });
  if (!preview) return <ContentSkeleton article />;
  return (
    <main className="reading-width pt-10 md:pt-14" aria-busy="true" aria-label="文章加载中">
      <div className="skeleton-line h-4 w-24" />
      <div className="article-reader mt-9">
        {preview.cover_url && (
          <ArticleCover
            key={preview.cover_url}
            src={preview.cover_url}
            title={preview.title}
            slug={preview.slug}
          />
        )}
        <header className="article-title-card">
          <h1 style={{ viewTransitionName: articleTransitionName("title", preview.slug) }}>
            {preview.title}
          </h1>
        </header>
        <p role="status" className="mt-5 text-xs text-muted">
          正在载入正文…
        </p>
        <div className="mt-14 space-y-4" aria-hidden="true">
          {Array.from({ length: 5 }, (_, index) => (
            <div
              key={index}
              className="skeleton-line h-4"
              style={{ width: `${index % 3 === 2 ? 68 : 100}%` }}
            />
          ))}
        </div>
      </div>
    </main>
  );
}
