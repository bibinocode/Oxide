import { useEffect, useMemo, useRef, useState } from "react";
import { EditorView } from "@codemirror/view";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";
import { readWritingStream } from "../writingStream";
import {
  setWritingReview,
  writingReviewExtension,
  type WritingReviewActions,
} from "../writingReview";

export type WritingAction = "draft" | "explain" | "improve" | "continue";

/** 捕获一次写作请求对应的完整原文和 UTF-16 选区坐标。 */
interface Snapshot {
  document: string;
  from: number;
  to: number;
  selected: string;
}

/** 请求前冻结选区和文档，阻止异步结果覆盖之后的编辑。 */
function snapshot(view: EditorView): Snapshot {
  const { from, to } = view.state.selection.main;
  return {
    document: view.state.doc.toString(),
    from,
    to,
    selected: view.state.doc.sliceString(from, to),
  };
}

/** 集中管理编辑器快捷键、候选请求与一次事务采纳。 */
export function useWritingAssistant(viewRef: React.RefObject<EditorView | null>) {
  const { session } = useAdminSession();
  const requestId = useRef(0);
  const controller = useRef<AbortController | null>(null);
  const reviewActions = useRef<WritingReviewActions>({
    accept: () => undefined,
    close: () => undefined,
  });
  const [selection, setSelection] = useState("");
  const [action, setAction] = useState<WritingAction | null>(null);
  const [origin, setOrigin] = useState<Snapshot | null>(null);
  const [instruction, setInstruction] = useState("");
  const [candidate, setCandidate] = useState("");
  const [pending, setPending] = useState(false);
  const [complete, setComplete] = useState(false);
  const [error, setError] = useState("");
  const [changed, setChanged] = useState(false);

  function close() {
    controller.current?.abort();
    requestId.current += 1;
    setAction(null);
    setOrigin(null);
    setInstruction("");
    setCandidate("");
    setPending(false);
    setComplete(false);
    setError("");
    setChanged(false);
  }

  function open(next: WritingAction) {
    const view = viewRef.current;
    if (!view) return;
    const captured = snapshot(view);
    if (next !== "draft" && !captured.selected.trim()) return;
    controller.current?.abort();
    requestId.current += 1;
    setAction(next);
    setOrigin(captured);
    setInstruction("");
    setCandidate("");
    setPending(false);
    setComplete(false);
    setError("");
    setChanged(false);
  }

  async function generate() {
    if (!session || !action || !origin || pending || changed) return;
    if (viewRef.current?.state.doc.toString() !== origin.document) {
      setChanged(true);
      return;
    }
    const current = ++requestId.current;
    controller.current?.abort();
    const active = new AbortController();
    controller.current = active;
    setPending(true);
    setComplete(false);
    setCandidate("");
    setError("");
    try {
      const response = await fetch("/api/v1/admin/agent/writing/stream", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", ...csrfHeaders(session) },
        signal: active.signal,
        body: JSON.stringify({
          action,
          instruction,
          selected: origin.selected,
          before: origin.document.slice(Math.max(0, origin.from - 6000), origin.from),
          after: origin.document.slice(origin.to, origin.to + 6000),
        }),
      });
      const content = await readWritingStream(response, (delta) => {
        if (requestId.current === current) setCandidate((previous) => previous + delta);
      });
      if (requestId.current === current) {
        setCandidate(content);
        setComplete(true);
      }
    } catch (cause) {
      if (requestId.current === current)
        setError(cause instanceof Error ? cause.message : "写作生成失败");
    } finally {
      if (requestId.current === current) {
        setPending(false);
        controller.current = null;
      }
    }
  }

  /** 一次事务完成采纳，CodeMirror 的撤销栈可恢复原文。 */
  function accept() {
    const view = viewRef.current;
    if (!view || !origin || !action || !complete || !candidate.trim()) return;
    if (view.state.doc.toString() !== origin.document) {
      setChanged(true);
      return;
    }
    if (action === "explain") return;
    const from =
      action === "improve" ? origin.from : action === "continue" ? origin.to : origin.from;
    const to = action === "improve" ? origin.to : from;
    const insert = action === "continue" ? `\n${candidate.trim()}` : candidate.trim();
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length },
    });
    close();
    view.focus();
  }

  reviewActions.current = { accept, close };
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: setWritingReview.of(
        origin && action && !changed && (pending || candidate)
          ? {
              from: origin.from,
              to: origin.to,
              original: origin.selected,
              candidate,
              action,
              pending,
              complete,
              changed,
            }
          : null,
      ),
    });
  }, [action, origin, candidate, pending, complete, changed, viewRef]);
  useEffect(() => () => controller.current?.abort(), []);

  const extensions = useMemo(
    () => [
      writingReviewExtension(reviewActions),
      EditorView.updateListener.of((update) => {
        if (update.selectionSet || update.docChanged) {
          const { from, to } = update.state.selection.main;
          setSelection(update.state.doc.sliceString(from, to));
        }
        if (update.docChanged) setChanged(true);
      }),
      EditorView.domEventHandlers({
        keydown(event, view) {
          if (
            event.key !== " " ||
            event.ctrlKey ||
            event.metaKey ||
            event.altKey ||
            event.repeat ||
            event.isComposing
          )
            return false;
          const { from, to } = view.state.selection.main;
          const line = view.state.doc.lineAt(from);
          if (from !== to || line.text.trim() || from !== line.from) return false;
          event.preventDefault();
          open("draft");
          return true;
        },
      }),
    ],
    [],
  );

  return {
    extensions,
    selection,
    action,
    origin,
    instruction,
    setInstruction,
    candidate,
    setCandidate,
    pending,
    complete,
    error,
    changed,
    open,
    close,
    generate,
    accept,
  };
}
