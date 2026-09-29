import { createFileRoute } from "@tanstack/react-router";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";

export const Route = createFileRoute("/about")({ component: About });

function About() {
  return (
    <main className="public-width pt-10 md:pt-14">
      <PublicPageHeader title="关于" />
      <div className="prose-blog mt-8 border-t border-line pt-4">
        <p>这里保存文章、思考与实践记录。内容围绕技术、设计与日常展开。</p>
        <p>
          通过 <a href="/feed.xml">RSS</a> 可以订阅更新。
        </p>
      </div>
    </main>
  );
}
