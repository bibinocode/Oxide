import { Link } from "@tanstack/react-router";
import type { ArticleSummary } from "../../../lib/api/types";
import { SITE_TIME_ZONE } from "../../../lib/utils/dateGroups";
import { ArticlePrint } from "./ArticlePrint";
import { articleTransitionName } from "../articleMotion";

const monthDay = new Intl.DateTimeFormat("zh-CN", {
  timeZone: SITE_TIME_ZONE,
  month: "2-digit",
  day: "2-digit",
});

export function ArticleRow({
  article,
  showCover = true,
}: {
  article: ArticleSummary;
  showCover?: boolean;
}) {
  return (
    <article>
      <Link
        to="/articles/$slug"
        params={{ slug: article.slug }}
        preload={false}
        viewTransition={{ types: ["article"] }}
        state={{
          articlePreview: {
            slug: article.slug,
            title: article.title,
            cover_url: article.cover_url,
          },
        }}
        className="catalog-row group"
      >
        {showCover && (
          <ArticlePrint
            key={article.cover_url}
            src={article.cover_url}
            transitionName={articleTransitionName("cover", article.slug)}
          />
        )}
        <span
          className="catalog-row-title transition-colors group-hover:text-ink"
          style={{ viewTransitionName: articleTransitionName("title", article.slug) }}
        >
          {article.title}
        </span>
        <span className="catalog-row-leader" aria-hidden="true" />
        <time className="catalog-row-date" dateTime={article.published_at ?? undefined}>
          {article.published_at ? monthDay.format(new Date(article.published_at)) : "未定"}
        </time>
      </Link>
    </article>
  );
}
