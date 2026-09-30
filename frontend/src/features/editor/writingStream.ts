import { ApiRequestError } from "../../lib/api/client";

type WritingEvent = "delta" | "done" | "error" | "reasoning" | "tool" | "status";

/** 只呈现服务端真实事件，不将普通文本推测为工具调用或思考过程。 */
export interface WritingProgress {
  id: string;
  kind: "reasoning" | "tool" | "status";
  content: string;
  delta?: boolean;
}

/** POST SSE 使用标准 fetch，以便发送 CSRF 头并支持 AbortController。 */
export async function readWritingStream(
  response: Response,
  onDelta: (text: string) => void,
  onProgress?: (progress: WritingProgress) => void,
): Promise<string> {
  if (!response.ok) {
    const failure = await response.json().catch(() => null);
    throw new ApiRequestError(
      failure?.message ?? `请求失败 (${response.status})`,
      failure?.code ?? "http_error",
      response.status,
    );
  }
  if (!response.body || !response.headers.get("content-type")?.includes("text/event-stream")) {
    throw new Error("服务未返回写作流");
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer = (buffer + decoder.decode(value, { stream: true })).replaceAll("\r\n", "\n");
      let boundary = buffer.indexOf("\n\n");
      while (boundary !== -1) {
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        const event = frame.match(/^event: (.+)$/m)?.[1] as WritingEvent | undefined;
        const data = frame.match(/^data: (.+)$/m)?.[1];
        if (event && data) {
          const payload = JSON.parse(data) as {
            content?: string;
            message?: string;
            id?: string;
            delta?: boolean;
          };
          if (event === "delta") onDelta(payload.content ?? "");
          if (event === "error") throw new Error(payload.message ?? "写作生成失败");
          if (event === "done") return payload.content ?? "";
          if (event === "reasoning" || event === "tool" || event === "status") {
            onProgress?.({
              id: payload.id ?? event,
              kind: event,
              content: payload.content ?? payload.message ?? "",
              delta: payload.delta,
            });
          }
        }
        boundary = buffer.indexOf("\n\n");
      }
    }
    throw new Error("写作流提前结束，请重试");
  } finally {
    await reader.cancel().catch(() => undefined);
  }
}
