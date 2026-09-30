import { useCallback, useEffect, useRef, useState } from "react";
import { apiRequest } from "../../../../lib/api/client";
import type { NotionPage, NotionPageList } from "../../../../lib/api/types";

interface NotionPagesResult {
  pages: NotionPage[];
  cursor: string | null;
  loading: boolean;
  error: string;
  setError: (message: string) => void;
  load: (term: string, nextCursor?: string) => Promise<void>;
}

/** Notion 页面搜索与分页；新请求取消旧请求，避免过期结果覆盖搜索列表。 */
export function useNotionPages(searchTerm: string): NotionPagesResult {
  const [pages, setPages] = useState<NotionPage[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const request = useRef<AbortController | null>(null);

  const load = useCallback(async (term: string, nextCursor?: string) => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setLoading(true);
    setError("");
    try {
      const params = new URLSearchParams();
      if (term) params.set("q", term);
      if (nextCursor) params.set("cursor", nextCursor);
      const result = await apiRequest<NotionPageList>(`/api/v1/admin/notion/pages?${params}`, {
        signal: controller.signal,
      });
      if (controller.signal.aborted) return;
      setPages((previous) => (nextCursor ? [...previous, ...result.items] : result.items));
      setCursor(result.next_cursor);
    } catch (cause) {
      if (!controller.signal.aborted)
        setError(cause instanceof Error ? cause.message : "读取 Notion 页面失败");
    } finally {
      if (!controller.signal.aborted) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(searchTerm);
    return () => request.current?.abort();
  }, [load, searchTerm]);

  return { pages, cursor, loading, error, setError, load };
}
