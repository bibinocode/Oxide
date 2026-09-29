import { useEffect, useState } from "react";
import { apiRequest } from "../../../lib/api/client";
import type { Session } from "../../../lib/api/types";
import { csrfHeaders } from "../../admin/AdminSession";

/** 防抖并取消过期预览，确保右侧 HTML 与正式文章由同一个解析器生成。 */
export function useArticlePreview(source: string, session: Session | null) {
  const [html, setHtml] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    if (!source) {
      setHtml("");
      setError("");
      return;
    }
    if (!session) return;
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      apiRequest<{ html: string }>("/api/v1/admin/preview", {
        method: "POST",
        headers: csrfHeaders(session),
        signal: controller.signal,
        body: JSON.stringify({ document: { type: "markdown", source } }),
      })
        .then((data) => {
          if (!controller.signal.aborted) {
            setHtml(data.html);
            setError("");
          }
        })
        .catch((cause) => {
          if (!controller.signal.aborted)
            setError(cause instanceof Error ? cause.message : "预览暂不可用");
        });
    }, 160);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [source, session]);
  return { html, error };
}
