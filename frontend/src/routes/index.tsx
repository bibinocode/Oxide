import { Link, createFileRoute } from "@tanstack/react-router";
import { ArticleRow } from "../features/article/components/ArticleRow";
import { PixelMark, SectionTag } from "../components/layout/PrintMarks";
import { getHomeData } from "../lib/api/server";
import { SiteSection } from "../components/layout/SiteSection";
import { ContentSkeleton } from "../components/layout/ContentSkeleton";
import { RevealContent } from "../components/layout/RevealContent";

export const Route = createFileRoute("/")({
  loader: () => getHomeData(),
  pendingComponent: ContentSkeleton,
  component: Home,
});

function Home() {
  const { site, articles, categories, tags } = Route.useLoaderData();
  return (
    <main className="public-width pt-10 md:pt-14">
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

      <section className="mt-16" aria-labelledby="writing-heading">
        <div className="flex items-center justify-between">
          <SectionTag index="01">
            <span id="writing-heading">写作</span>
          </SectionTag>
          <Link to="/archive" search={{ page: 1 }} className="text-sm text-muted hover:text-ink">
            查看全部
          </Link>
        </div>
        <RevealContent className="mt-4">
          {articles.items.length ? (
            articles.items
              .slice(0, 6)
              .map((article) => <ArticleRow key={article.public_id} article={article} />)
          ) : (
            <p className="border-t border-line py-8 text-sm text-muted">还没有已发布的文章。</p>
          )}
        </RevealContent>
      </section>

      <section className="mt-16" aria-labelledby="categories-heading">
        <SectionTag index="02">
          <span id="categories-heading">分类</span>
        </SectionTag>
        <RevealContent className="mt-4 border-t border-line">
          {categories.length ? (
            categories.map((item) => (
              <Link
                key={item.slug}
                to="/categories/$slug"
                params={{ slug: item.slug }}
                search={{ page: 1 }}
                className="catalog-row group"
              >
                <span className="catalog-row-title group-hover:text-ink">{item.name}</span>
                <span className="catalog-row-leader" aria-hidden="true" />
                <span className="font-mono text-xs text-muted">/{item.slug}</span>
              </Link>
            ))
          ) : (
            <p className="py-6 text-sm text-muted">暂无分类。</p>
          )}
        </RevealContent>
      </section>

      {tags.length > 0 && (
        <section className="mt-16" aria-labelledby="tags-heading">
          <SectionTag index="03">
            <span id="tags-heading">标签</span>
          </SectionTag>
          <div className="mt-4 flex flex-wrap gap-x-5 gap-y-3 border-t border-line pt-5 text-sm">
            {tags.map((item) => (
              <Link
                key={item.slug}
                to="/tags/$slug"
                params={{ slug: item.slug }}
                search={{ page: 1 }}
                className="text-muted hover:text-ink"
              >
                #{item.name}
              </Link>
            ))}
          </div>
        </section>
      )}

      <SiteSection
        id="projects"
        title="项目作品集"
        index="04"
        section={site.presentation?.projects}
      />
      <SiteSection id="services" title="服务" index="05" section={site.presentation?.services} />
      <p className="mt-16 border-t border-line pt-5 font-mono text-xs text-muted">
        {articles.total} 篇文章 ·{" "}
        <a href="/feed.xml" className="hover:text-ink">
          RSS 订阅
        </a>
      </p>
    </main>
  );
}
