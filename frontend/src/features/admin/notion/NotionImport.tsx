import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowUpRight, RefreshCw, Search } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../AdminSession";
import { ApiRequestError, apiRequest } from "../../../lib/api/client";
import type { NotionPage, NotionPageList, NotionSyncResult } from "../../../lib/api/types";

export function NotionImport() {
  const { session } = useAdminSession();
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [searchTerm, setSearchTerm] = useState("");
  const [pages, setPages] = useState<NotionPage[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const load = useCallback(async (term: string, nextCursor?: string) => {
    setLoading(true);
    setError("");
    try {
      const params = new URLSearchParams();
      if (term) params.set("q", term);
      if (nextCursor) params.set("cursor", nextCursor);
      const result = await apiRequest<NotionPageList>(`/api/v1/admin/notion/pages?${params}`);
      setPages((previous) => (nextCursor ? [...previous, ...result.items] : result.items));
      setCursor(result.next_cursor);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "读取 Notion 页面失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(searchTerm);
  }, [load, searchTerm]);

  async function sync(page: NotionPage, force = false): Promise<void> {
    if (!session) return;
    setBusyId(page.id);
    setError("");
    setNotice("");
    try {
      const result = await apiRequest<NotionSyncResult>(
        `/api/v1/admin/notion/pages/${page.id}/sync`,
        {
          method: "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({ force }),
        },
      );
      if (result.warnings.length) {
        setNotice(`导入已完成，请检查转换结果：${result.warnings.join("；")}`);
        await load(searchTerm);
        return;
      }
      await navigate({
        to: "/admin/articles/$publicId",
        params: { publicId: result.article.public_id },
      });
    } catch (cause) {
      if (
        cause instanceof ApiRequestError &&
        cause.code === "notion_local_changes" &&
        window.confirm(
          "这篇文章在本站修改过。继续同步会用 Notion 正文和摘要覆盖本站版本，原正文会保留在修订历史中。确定继续？",
        )
      ) {
        await sync(page, true);
        return;
      }
      setError(cause instanceof Error ? cause.message : "同步失败");
    } finally {
      setBusyId(null);
    }
  }

  return (
    <section className="pt-9">
      <div className="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-7">
        <div>
          <p className="eyebrow">Content / Notion</p>
          <h2 className="mt-2 text-base font-semibold">从 Notion 导入</h2>
          <p className="mt-2 text-sm text-muted">选择已共享给集成的页面。首次导入保存为草稿。</p>
        </div>
        <button
          className="button-secondary"
          type="button"
          disabled={loading}
          onClick={() => void load(searchTerm)}
          title="刷新页面列表"
        >
          <RefreshCw size={16} /> 刷新
        </button>
      </div>
      <form
        className="mt-7 flex max-w-xl gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          setSearchTerm(query.trim());
        }}
      >
        <input
          className="field min-w-0"
          aria-label="搜索 Notion 页面标题"
          placeholder="搜索页面标题"
          value={query}
          maxLength={100}
          onChange={(event) => setQuery(event.target.value)}
        />
        <button className="button-primary shrink-0" type="submit">
          <Search size={16} /> 搜索
        </button>
      </form>
      {error && (
        <p className="mt-5 text-sm text-warm" role="alert">
          {error}
        </p>
      )}
      {notice && (
        <p className="mt-5 text-sm text-muted" role="status">
          {notice}
        </p>
      )}
      <div className="mt-7 border-t border-line">
        {pages.map((page) => (
          <div
            key={page.id}
            className="flex flex-wrap items-center justify-between gap-4 border-b border-line py-5"
          >
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-3">
                <span className="font-medium">{page.title}</span>
                {page.article_public_id && (
                  <span className="font-mono text-xs text-muted">
                    {page.article_status === "published" ? "已发布" : "草稿"}
                  </span>
                )}
              </div>
              <p className="mt-2 text-xs text-muted">
                Notion 更新于 {new Date(page.last_edited_at).toLocaleString("zh-CN")}
                {page.local_changes ? " · 本站有修改" : ""}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <a
                href={page.url}
                target="_blank"
                rel="noopener noreferrer"
                className="button-secondary"
                title="在 Notion 中打开"
              >
                <ArrowUpRight size={16} />
              </a>
              {page.article_public_id && (
                <Link
                  to="/admin/articles/$publicId"
                  params={{ publicId: page.article_public_id }}
                  className="button-secondary"
                >
                  编辑
                </Link>
              )}
              <button
                type="button"
                className="button-primary"
                disabled={busyId !== null}
                onClick={() => void sync(page)}
              >
                {busyId === page.id ? "同步中…" : page.article_public_id ? "同步" : "导入"}
              </button>
            </div>
          </div>
        ))}
        {!loading && !error && pages.length === 0 && (
          <p className="py-10 text-sm text-muted">没有找到已授权的 Notion 页面。</p>
        )}
      </div>
      {loading && (
        <p className="mt-5 text-sm text-muted" role="status">
          正在读取页面…
        </p>
      )}
      {cursor && !loading && (
        <button
          className="button-secondary mt-5"
          type="button"
          onClick={() => void load(searchTerm, cursor)}
        >
          加载更多
        </button>
      )}
    </section>
  );
}
