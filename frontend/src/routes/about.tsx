import { createFileRoute } from "@tanstack/react-router";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { getSiteData } from "../lib/api/server";
import Markdown from "react-markdown";

export const Route = createFileRoute("/about")({ loader: () => getSiteData(), component: About });

/** 关于正文与首页简介独立管理，不再展示写死的示例内容。 */
function About() {
  const site = Route.useLoaderData();
  const body = site.presentation?.about_body?.trim();
  return (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="关于" />
      <div className="prose-blog mt-8 border-t border-line pt-4">
        {body ? (
          <Markdown>{body}</Markdown>
        ) : (
          <p className="text-muted">关于我的内容正在整理中。</p>
        )}
      </div>
    </main>
  );
}
