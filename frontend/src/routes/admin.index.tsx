import { Link, createFileRoute } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { ArticleManagementList } from "../features/admin/articles/ArticleManagementList";

export const Route = createFileRoute("/admin/")({ component: AdminArticles });
/** 文章列表直接提供发布、下架和删除入口。 */
function AdminArticles() {
  return (
    <section className="pt-9">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h2 className="text-base font-semibold">文章</h2>
          <p className="mt-2 text-sm text-muted">草稿、已发布和已下架内容</p>
        </div>
        <Link to="/admin/articles/new" className="button-primary">
          <Plus size={17} /> 新建文章
        </Link>
      </div>
      <ArticleManagementList />
    </section>
  );
}
