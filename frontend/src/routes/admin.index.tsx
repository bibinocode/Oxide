import { Link, createFileRoute } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { apiRequest } from "../lib/api/client";
import type { AdminArticle } from "../lib/api/types";

export const Route = createFileRoute("/admin/")({ component: AdminArticles });

function AdminArticles() {
  const [articles, setArticles] = useState<AdminArticle[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    apiRequest<{ items: AdminArticle[] }>("/api/v1/admin/articles")
      .then((data) => setArticles(data.items))
      .catch((cause) => setError(cause.message));
  }, []);
  return (
    <section className="pt-9">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h2 className="text-base font-semibold">文章</h2>
          <p className="mt-2 text-sm text-muted">草稿与已发布内容</p>
        </div>
        <Link to="/admin/articles/new" className="button-primary">
          <Plus size={17} /> 新建文章
        </Link>
      </div>
      <p role="alert" className="mt-4 text-sm text-warm">
        {error}
      </p>
      <div className="mt-8 border-t border-line">
        {articles.map((article) => (
          <Link
            key={article.public_id}
            to="/admin/articles/$publicId"
            params={{ publicId: article.public_id }}
            className="grid gap-2 border-b border-line py-5 hover:text-accent md:grid-cols-[1fr_100px_140px]"
          >
            <span className="font-medium">{article.title}</span>
            <span className="text-sm text-muted">
              {article.status === "published" ? "已发布" : "草稿"}
            </span>
            <time className="text-sm text-muted">
              {new Intl.DateTimeFormat("zh-CN").format(new Date(article.updated_at))}
            </time>
          </Link>
        ))}
        {articles.length === 0 && !error && <p className="py-10 text-sm text-muted">暂无文章。</p>}
      </div>
    </section>
  );
}
