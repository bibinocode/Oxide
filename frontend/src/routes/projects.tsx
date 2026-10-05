import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { ArrowUpRight, FolderCode } from "lucide-react";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { ProjectStage } from "../features/projects/ProjectStage";
import { ProjectSchematic } from "../features/projects/ProjectSchematic";
import { getSiteData } from "../lib/api/server";

export const Route = createFileRoute("/projects")({
  loader: async () => {
    const site = await getSiteData();
    if (!site.presentation?.projects.enabled) throw notFound();
    return site.presentation.projects;
  },
  head: () => ({ meta: [{ title: "项目作品集 · Oxide" }] }),
  notFoundComponent: () => (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="作品集未开放" detail="这个模块暂未开放。" />
      <Link to="/" className="button-secondary mt-8">
        返回首页
      </Link>
    </main>
  ),
  component: Projects,
});

/** 从项目链接提取展示域名；站内路径保留路径，非网页协议不伪造域名。 */
function projectAddress(value: string): string {
  if (value.startsWith("/")) return value;
  try {
    return new URL(value).hostname.replace(/^www\./, "") || value;
  } catch {
    return value;
  }
}

/** 独立作品集按管理端顺序展示，沿用参考站的细线与三列项目布局。 */
function Projects() {
  const { items } = Route.useLoaderData();
  return (
    <main className="projects-page public-width pt-10 md:pt-14">
      <ProjectSchematic />
      <PublicPageHeader title="项目" detail="构建过的产品、工具与实验。" />
      {items.length ? (
        <ProjectStage>
          <ul className="projects-list">
            {items.map((item, index) => (
              <li key={`${item.url}-${index}`}>
                <a className="project-row" href={item.url} rel="noopener noreferrer">
                  <span className="project-icon-frame" aria-hidden="true">
                    <span className="project-icon">
                      {item.avatar_url ? (
                        <img src={item.avatar_url} alt="" width={36} height={36} loading="lazy" />
                      ) : (
                        <FolderCode size={24} strokeWidth={1.25} />
                      )}
                    </span>
                  </span>
                  <span className="project-identity">
                    <span className="project-name">
                      {item.title}
                      {!item.url.startsWith("/") && (
                        <ArrowUpRight className="project-external" size={13} aria-hidden="true" />
                      )}
                    </span>
                    <span className="project-domain">{projectAddress(item.url)}</span>
                  </span>
                  <span className="project-description">{item.description}</span>
                </a>
              </li>
            ))}
          </ul>
        </ProjectStage>
      ) : (
        <p className="mt-10 border-t border-line py-8 text-sm text-muted">项目正在整理中。</p>
      )}
    </main>
  );
}
