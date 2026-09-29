import { Link } from "@tanstack/react-router";
import type { ArticleSummary } from "../../../lib/api/types";

export function ArticleRow({ article }: { article: ArticleSummary }) {
  return (
    <article>
      <Link
        to="/articles/$slug"
        params={{ slug: article.slug }}
        preload={false}
        className="catalog-row group"
      >
        <span className="catalog-row-title transition-colors group-hover:text-ink">
          {article.title}
        </span>
        <span className="catalog-row-leader" aria-hidden="true" />
        <time className="catalog-row-date" dateTime={article.published_at ?? undefined}>
          {article.published_at
            ? new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit" }).format(
                new Date(article.published_at),
              )
            : "未定"}
        </time>
      </Link>
    </article>
  );
}
