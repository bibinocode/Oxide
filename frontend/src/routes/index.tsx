import { Link, createFileRoute } from "@tanstack/react-router";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PixelMark, SectionTag } from "../components/layout/PrintMarks";
import { getHomeData } from "../lib/api/server";
import { SiteSection } from "../components/layout/SiteSection";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { HomeIntroduction } from "../features/home/HomeIntroduction";
import { ColumnBookshelf } from "../features/columns/components/ColumnBookshelf";
import { HomeContentSummary } from "../features/home/HomeContentSummary";

export const Route = createFileRoute("/")({
  loader: () => getHomeData(),
  pendingComponent: ContentSkeleton,
  component: Home,
});

function Home() {
  const { site, articles, columns } = Route.useLoaderData();
  const latest = articles.items.slice(0, 6);
  const center = (latest.length - 1) / 2;
  return (
    <main className="public-width pt-10 md:pt-14">
      {site.presentation?.home_intro?.enabled ? (
        <HomeIntroduction
          name={site.site_name}
          description={site.description}
          value={site.presentation.home_intro}
        />
      ) : (
        <section aria-labelledby="site-title">
          <div className="flex items-center gap-3">
            <h1 id="site-title" className="text-base font-semibold text-ink">
              {site.site_name}
            </h1>
            <PixelMark />
          </div>
          <p className="mt-5 max-w-[34rem] text-sm leading-7 text-muted">
            {site.description ?? "关于技术、设计与日常的记录。"}
          </p>
        </section>
      )}

      <HomeContentSummary
        articleCount={articles.total}
        columnCount={columns.length}
        projects={site.presentation?.projects}
      />

      <section className="mt-16" aria-labelledby="writing-heading">
        <div className="flex items-center justify-between">
          <SectionTag index="01">
            <span id="writing-heading">写作</span>
          </SectionTag>
          <Link to="/archive" search={{ page: 1 }} className="text-sm text-muted hover:text-ink">
            查看全部
          </Link>
        </div>
        <div className="home-writing-list mt-4">
          {latest.length ? (
            latest.map((article, index) => (
              <div
                key={article.public_id}
                className="home-writing-enter"
                style={{ animationDelay: `${240 + Math.abs(index - center) * 50}ms` }}
              >
                <ArticleRow article={article} showCover />
              </div>
            ))
          ) : (
            <p className="border-t border-line py-8 text-sm text-muted">还没有已发布的文章。</p>
          )}
        </div>
      </section>

      <section className="mt-16" aria-labelledby="columns-heading">
        <div className="flex items-center justify-between">
          <SectionTag index="02">
            <span id="columns-heading">小册</span>
          </SectionTag>
          <Link to="/columns" className="text-sm text-muted hover:text-ink">
            查看全部
          </Link>
        </div>
        <div className="mt-6">
          <ColumnBookshelf columns={columns.slice(0, 6)} showDetails={false} />
        </div>
      </section>

      <SiteSection id="services" title="服务" index="03" section={site.presentation?.services} />
    </main>
  );
}
