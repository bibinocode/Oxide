import { useCallback, useEffect, useRef, useState } from "react";
import { apiRequest } from "../../../../lib/api/client";
import type { AdminArticle } from "../../../../lib/api/types";
import { csrfHeaders, useAdminSession } from "../../AdminSession";

interface ArticlePage {
  items: AdminArticle[];
  total: number;
}
interface AdminArticlesState {
  data: ArticlePage | null;
  page: number;
  loading: boolean;
  busy: string | null;
  error: string;
  setPage: (page: number) => void;
  reload: () => void;
  act: (article: AdminArticle, action: "publish" | "unpublish" | "delete") => Promise<void>;
}
/** 全部文章与小册目录共享分页和管理动作；请求清理和同步锁防止旧结果覆盖或重复提交。 */
export function useAdminArticles(columnId?: string): AdminArticlesState {
  const { session } = useAdminSession();
  const [data, setData] = useState<ArticlePage | null>(null);
  const [page, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const lock = useRef(false);
  const reload = useCallback(() => setRevision((value) => value + 1), []);
  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError("");
    const query = new URLSearchParams({ page: String(page) });
    if (columnId) query.set("column_public_id", columnId);
    apiRequest<ArticlePage>(`/api/v1/admin/articles?${query}`, { signal: controller.signal })
      .then((result) => {
        if (controller.signal.aborted) return;
        if (page > 1 && result.items.length === 0) setPage(page - 1);
        else setData(result);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : "文章加载失败");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [page, columnId, revision]);
  async function act(article: AdminArticle, action: "publish" | "unpublish" | "delete") {
    if (!session || loading || lock.current) return;
    lock.current = true;
    setBusy(article.public_id);
    setError("");
    try {
      const endpoint = `/api/v1/admin/articles/${article.public_id}`;
      await apiRequest(action === "delete" ? endpoint : `${endpoint}/${action}`, {
        method: action === "delete" ? "DELETE" : "POST",
        headers: csrfHeaders(session),
      });
      reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "操作失败");
    } finally {
      lock.current = false;
      setBusy(null);
    }
  }
  return { data, page, loading, busy, error, setPage, reload, act };
}
