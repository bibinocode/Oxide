import { Link } from "@tanstack/react-router";
import { useAdminArticles } from "./hooks/useAdminArticles";

/** 管理动作保持可见，不藏在编辑页折叠菜单中；删除前明确告知关联数据范围。 */
export function ArticleManagementList({ columnId }: { columnId?: string }) {
  const { data, page, loading, busy, error, setPage, reload, act } = useAdminArticles(columnId);
  return (
    <section className="mt-8" aria-label={columnId ? "小册文章目录" : "文章管理列表"}>
      <div className="flex items-center justify-between gap-3 border-b border-line pb-4">
        <h2 className="text-sm font-medium">
          {columnId ? "文章目录" : "全部文章"}
          {data && `（${data.total} 篇）`}
        </h2>
        <button
          type="button"
          className="button-secondary"
          disabled={loading || !!busy}
          onClick={reload}
        >
          刷新列表
        </button>
      </div>
      <p className="mt-3 text-xs leading-6 text-muted">
        下架后公开页面不再展示，正文仍可编辑并重新发布；删除会永久移除文章、评论和修订记录。
      </p>
      {error && (
        <p role="alert" className="mt-3 text-sm text-warm">
          {error}
        </p>
      )}
      {loading ? (
        <p role="status" className="py-8 text-sm text-muted">
          正在加载文章…
        </p>
      ) : (
        <div>
          {data?.items.map((article) => (
            <article
              key={article.public_id}
              className="grid gap-3 border-b border-line py-5 md:grid-cols-[minmax(0,1fr)_auto]"
            >
              <div className="min-w-0">
                <Link
                  to="/admin/articles/$publicId"
                  params={{ publicId: article.public_id }}
                  className="break-words text-sm font-medium hover:underline"
                >
                  {article.title}
                </Link>
                <p className="mt-2 text-xs text-muted">
                  {article.status === "published"
                    ? "已发布"
                    : article.published_at
                      ? "已下架"
                      : "草稿"}{" "}
                  · {new Intl.DateTimeFormat("zh-CN").format(new Date(article.updated_at))}
                  {columnId && ` · ${article.access.subscriber_only ? "付费" : "免费"}`}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Link
                  to="/admin/articles/$publicId"
                  params={{ publicId: article.public_id }}
                  className="button-secondary"
                >
                  编辑
                </Link>
                <button
                  type="button"
                  className="button-secondary"
                  disabled={loading || !!busy}
                  onClick={() =>
                    void act(article, article.status === "published" ? "unpublish" : "publish")
                  }
                >
                  {busy === article.public_id
                    ? "处理中…"
                    : article.status === "published"
                      ? "下架 / 隐藏"
                      : "发布"}
                </button>
                <button
                  type="button"
                  className="button-secondary text-warm"
                  disabled={loading || !!busy}
                  onClick={() => {
                    if (
                      window.confirm(
                        `永久删除「${article.title}」及其评论、修订记录？此操作不可撤销。`,
                      )
                    )
                      void act(article, "delete");
                  }}
                >
                  删除
                </button>
              </div>
            </article>
          ))}
          {data?.total === 0 && <p className="py-8 text-sm text-muted">暂无文章。</p>}
        </div>
      )}
      {data && data.total > 20 && (
        <nav aria-label="文章分页" className="mt-5 flex items-center gap-3 text-sm">
          <button
            type="button"
            className="button-secondary"
            disabled={loading || !!busy || page === 1}
            onClick={() => setPage(page - 1)}
          >
            上一页
          </button>
          <span>
            {page} / {Math.ceil(data.total / 20)}
          </span>
          <button
            type="button"
            className="button-secondary"
            disabled={loading || !!busy || page * 20 >= data.total}
            onClick={() => setPage(page + 1)}
          >
            下一页
          </button>
        </nav>
      )}
    </section>
  );
}
