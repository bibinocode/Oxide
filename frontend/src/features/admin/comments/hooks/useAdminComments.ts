import { useEffect, useRef, useState, useCallback } from "react";
import { apiRequest } from "../../../../lib/api/client";
import { csrfHeaders, useAdminSession } from "../../AdminSession";
import type { AdminComment } from "../types";

type Action = "approved" | "rejected" | "pending" | "delete" | "agent";
interface AdminCommentsState {
  comments: AdminComment[];
  page: number;
  setPage: (page: number) => void;
  loading: boolean;
  busy: string | null;
  error: string;
  reload: () => void;
  act: (comment: AdminComment, action: Action) => Promise<void>;
}
/** 自动刷新 Agent 结果；人工操作期间取消旧列表请求，避免过期读结果盖过新状态。 */
export function useAdminComments(filter: string): AdminCommentsState {
  const { session } = useAdminSession();
  const [page, setPage] = useState(1);
  const [comments, setComments] = useState<AdminComment[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const lock = useRef(false);
  const read = useRef<AbortController | null>(null);
  const reload = useCallback(() => setRevision((value) => value + 1), []);
  useEffect(() => {
    let disposed = false;
    let running = false;
    async function load(initial = false) {
      if (disposed || running || lock.current || (!initial && document.hidden)) return;
      running = true;
      const controller = new AbortController();
      read.current = controller;
      if (initial) {
        setLoading(true);
        setError("");
      }
      try {
        const result = await apiRequest<AdminComment[]>(
          `/api/v1/admin/comments?page=${page}&status=${filter}`,
          {
            signal: controller.signal,
          },
        );
        if (!disposed && !controller.signal.aborted) setComments(result);
      } catch (cause) {
        if (!disposed && !controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : "评论加载失败");
      } finally {
        running = false;
        if (!disposed && initial) setLoading(false);
      }
    }
    void load(true);
    const timer = window.setInterval(() => void load(), 5000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      read.current?.abort();
    };
  }, [revision, page, filter]);
  async function act(comment: AdminComment, action: Action) {
    if (!session || lock.current) return;
    lock.current = true;
    read.current?.abort();
    setBusy(comment.public_id);
    setError("");
    try {
      const path = `/api/v1/admin/comments/${comment.public_id}`;
      if (action === "delete") {
        await apiRequest(path, { method: "DELETE", headers: csrfHeaders(session) });
        setComments((items) => items.filter((item) => item.public_id !== comment.public_id));
      } else {
        const updated = await apiRequest<AdminComment>(
          action === "agent" ? `${path}/agent-review` : path,
          {
            method: action === "agent" ? "POST" : "PATCH",
            headers: csrfHeaders(session),
            body: action === "agent" ? undefined : JSON.stringify({ status: action }),
          },
        );
        setComments((items) =>
          items.map((item) => (item.public_id === comment.public_id ? updated : item)),
        );
      }
      reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "评论操作失败");
    } finally {
      lock.current = false;
      setBusy(null);
    }
  }
  return { comments, page, setPage, loading, busy, error, reload, act };
}
