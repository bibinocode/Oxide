import { useEffect, useRef, useState } from "react";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";
import { apiRequest } from "../../../lib/api/client";

/** 管理摘要候选与请求状态；标题或正文变化后旧候选自动失效。 */
export function useArticleSummary(title: string, source: string) {
  const { session } = useAdminSession();
  const requestId = useRef(0);
  const [candidate, setCandidate] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    requestId.current += 1;
    setCandidate("");
    setError("");
    setPending(false);
  }, [title, source]);

  async function generate() {
    if (!session || !title.trim() || !source.trim()) return;
    const current = ++requestId.current;
    setPending(true);
    setError("");
    setCandidate("");
    try {
      const result = await apiRequest<{ summary: string }>("/api/v1/admin/agent/summary", {
        method: "POST",
        headers: csrfHeaders(session),
        body: JSON.stringify({ title, source }),
      });
      if (requestId.current === current) setCandidate(result.summary);
    } catch (cause) {
      if (requestId.current === current)
        setError(cause instanceof Error ? cause.message : "摘要生成失败");
    } finally {
      if (requestId.current === current) setPending(false);
    }
  }

  function dismiss() {
    setCandidate("");
  }

  return { candidate, pending, error, generate, dismiss };
}
