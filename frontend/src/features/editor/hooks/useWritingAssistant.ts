import { useEffect, useMemo, useRef, useState } from "react";
import { EditorView } from "@codemirror/view";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";
import { readWritingStream, type WritingProgress } from "../writingStream";
import {
  setWritingReview,
  writingReviewExtension,
  type WritingReviewActions,
} from "../writingReview";
import {
  setWritingPrompt,
  writingPromptExtension,
  type WritingPromptActions,
} from "../writingPrompt";

export type WritingAction = "draft" | "explain" | "improve" | "continue";
export type ConversationMode = "sidebar" | "floating" | "fullscreen";
/** 审阅冻结原文；对话插入点独立映射，允许对话期间继续编辑。 */
interface Snapshot {
  document: string;
  from: number;
  to: number;
  selected: string;
}
export interface WritingMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
  progress: WritingProgress[];
  status: "streaming" | "done" | "stopped" | "error";
}
const labels = {
  draft: "使用 AI 编写…",
  explain: "解释所选内容，或补充你的问题…",
  improve: "提升写作，或补充修改要求…",
  continue: "续写所选内容，或补充要求…",
};

/** 管理原位输入、多轮上下文、流式回复和安全采纳；关闭面板保留当前会话。 */
export function useWritingAssistant(viewRef: React.RefObject<EditorView | null>) {
  const { session } = useAdminSession();
  const requestId = useRef(0);
  const controller = useRef<AbortController | null>(null);
  const insertionPoint = useRef(0);
  const sequence = useRef(0);
  const reviewActions = useRef<WritingReviewActions>({
    accept: () => undefined,
    close: () => undefined,
  });
  const promptActions = useRef<WritingPromptActions>({
    input: () => undefined,
    generate: () => undefined,
    close: () => undefined,
    stop: () => undefined,
  });
  const commands = useRef({ open: (_action: WritingAction) => {}, documentChanged: () => {} });
  const [selection, setSelection] = useState("");
  const [selectionAnchor, setSelectionAnchor] = useState<{ left: number; top: number } | null>(
    null,
  );
  const [action, setAction] = useState<WritingAction | null>(null);
  const [origin, setOrigin] = useState<Snapshot | null>(null);
  const [instruction, setInstruction] = useState("");
  const instructionRef = useRef("");
  const [candidate, setCandidate] = useState("");
  const [pending, setPending] = useState(false);
  const [complete, setComplete] = useState(false);
  const [error, setError] = useState("");
  const [changed, setChanged] = useState(false);
  const [messages, setMessages] = useState<WritingMessage[]>([]);
  const [panelOpen, setPanelOpen] = useState(false);
  const [mode, setMode] = useState<ConversationMode>("sidebar");
  const [inlineOpen, setInlineOpen] = useState(false);

  function updateInstruction(value: string) {
    instructionRef.current = value;
    setInstruction(value);
  }
  function dismissReview() {
    setAction(null);
    setOrigin(null);
    setCandidate("");
    setComplete(false);
    setChanged(false);
  }
  function closePrompt() {
    setInlineOpen(false);
    dismissReview();
    updateInstruction("");
  }
  function stop() {
    controller.current?.abort();
    controller.current = null;
    requestId.current += 1;
    setPending(false);
    setMessages((items) =>
      items.map((item) => (item.status === "streaming" ? { ...item, status: "stopped" } : item)),
    );
  }
  function newConversation() {
    stop();
    closePrompt();
    setMessages([]);
    setError("");
  }

  function open(next: WritingAction) {
    const view = viewRef.current;
    if (!view || pending) return;
    const { from, to } = view.state.selection.main;
    const captured = {
      document: view.state.doc.toString(),
      from,
      to,
      selected: view.state.doc.sliceString(from, to),
    };
    if (next !== "draft" && !captured.selected.trim()) return;
    insertionPoint.current = to;
    setAction(next);
    setOrigin(captured);
    updateInstruction("");
    setCandidate("");
    setComplete(false);
    setError("");
    setChanged(false);
    setInlineOpen(true);
  }

  async function generate() {
    if (!session || pending || controller.current) return;
    const requestedAction = inlineOpen && action ? action : "draft";
    const prompt = instructionRef.current.trim();
    if (requestedAction === "draft" && !prompt) return;
    const view = viewRef.current;
    if (!view) return;
    if (
      inlineOpen &&
      requestedAction !== "draft" &&
      (!origin || changed || view.state.doc.toString() !== origin.document)
    ) {
      setError("原文已变化，请重新选择后操作。");
      setPanelOpen(true);
      return;
    }
    if (!inlineOpen) {
      dismissReview();
      insertionPoint.current = view.state.selection.main.to;
    }
    const document = view.state.doc.toString();
    const from = inlineOpen && origin ? origin.from : insertionPoint.current;
    const to = inlineOpen && origin ? origin.to : from;
    const current = ++requestId.current;
    const active = new AbortController();
    controller.current = active;
    const replyId = ++sequence.current;
    const questionId = ++sequence.current;
    const history = messages
      .filter(
        (item, index) =>
          item.status === "done" &&
          (item.role === "assistant" || messages[index + 1]?.status === "done"),
      )
      .slice(-12)
      .map(({ role, content }) => ({ role, content }));
    while (history.reduce((total, item) => total + item.content.length, 0) > 40_000)
      history.splice(0, 2);
    setMessages((items) => [
      ...items.slice(-38),
      {
        id: questionId,
        role: "user",
        content: prompt || labels[requestedAction].replace("…", ""),
        progress: [],
        status: "done",
      },
      { id: replyId, role: "assistant", content: "", progress: [], status: "streaming" },
    ]);
    setInlineOpen(false);
    setPanelOpen(true);
    setPending(true);
    setComplete(false);
    setCandidate("");
    setError("");
    updateInstruction("");
    const updateReply = (update: (message: WritingMessage) => WritingMessage) =>
      setMessages((items) => items.map((item) => (item.id === replyId ? update(item) : item)));
    try {
      const response = await fetch("/api/v1/admin/agent/writing/stream", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", ...csrfHeaders(session) },
        signal: active.signal,
        body: JSON.stringify({
          action: requestedAction,
          instruction: prompt,
          selected: inlineOpen ? (origin?.selected ?? "") : "",
          before: document.slice(Math.max(0, from - 6000), from),
          after: document.slice(to, to + 6000),
          history,
        }),
      });
      const content = await readWritingStream(
        response,
        (delta) => {
          if (requestId.current !== current) return;
          setCandidate((previous) => previous + delta);
          updateReply((item) => ({ ...item, content: item.content + delta }));
        },
        (progress) => {
          if (requestId.current !== current) return;
          updateReply((item) => {
            const previous = item.progress.find(
              (entry) => entry.id === progress.id && entry.kind === progress.kind,
            );
            const next =
              previous && progress.delta
                ? { ...progress, content: previous.content + progress.content }
                : progress;
            return {
              ...item,
              progress: [
                ...item.progress.filter(
                  (entry) => !(entry.id === progress.id && entry.kind === progress.kind),
                ),
                next,
              ],
            };
          });
        },
      );
      if (requestId.current === current) {
        setCandidate(content);
        setComplete(true);
        updateReply((item) => ({ ...item, content, status: "done" }));
      }
    } catch (cause) {
      if (requestId.current === current) {
        setError(cause instanceof Error ? cause.message : "写作生成失败");
        updateReply((item) => ({ ...item, status: "error" }));
      }
    } finally {
      if (requestId.current === current) {
        setPending(false);
        controller.current = null;
      }
    }
  }

  /** 一次事务采纳已完成候选，原文变化时拒绝覆盖，支持编辑器撤销。 */
  function accept() {
    const view = viewRef.current;
    if (
      !view ||
      !origin ||
      !complete ||
      pending ||
      !candidate.trim() ||
      (action !== "improve" && action !== "continue")
    )
      return;
    if (view.state.doc.toString() !== origin.document) {
      setChanged(true);
      return;
    }
    const from = action === "improve" ? origin.from : origin.to;
    const to = action === "improve" ? origin.to : from;
    const insert = action === "continue" ? "\n" + candidate.trim() : candidate.trim();
    view.dispatch({ changes: { from, to, insert }, selection: { anchor: from + insert.length } });
    dismissReview();
    view.focus();
  }

  /** 回复只在映射后的插入点新增内容，不隐式替换选区或销毁历史。 */
  function insertReply(content: string) {
    const view = viewRef.current;
    if (!view || !content.trim()) return;
    const from = Math.min(insertionPoint.current, view.state.doc.length);
    const before = view.state.doc.sliceString(0, from);
    const after = view.state.doc.sliceString(from);
    const insert =
      (before && !before.endsWith("\n") ? "\n\n" : "") +
      content.trim() +
      (after && !after.startsWith("\n") ? "\n\n" : "");
    view.dispatch({ changes: { from, insert }, selection: { anchor: from + insert.length } });
    dismissReview();
    view.focus();
  }

  reviewActions.current = { accept, close: dismissReview };
  promptActions.current = {
    input: updateInstruction,
    generate: () => void generate(),
    close: closePrompt,
    stop,
  };
  commands.current = {
    open,
    documentChanged: () => {
      if (origin) {
        setChanged(true);
        setInlineOpen(false);
      }
    },
  };
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: [
        setWritingPrompt.of(
          origin &&
            (inlineOpen || (pending && (action === "draft" || action === "explain"))) &&
            !changed
            ? {
                position: origin.to,
                label: labels[action ?? "draft"],
                pending: !inlineOpen && pending,
              }
            : null,
        ),
        setWritingReview.of(
          origin &&
            (action === "improve" || action === "continue") &&
            !changed &&
            (pending || candidate)
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
      ],
    });
  }, [action, origin, inlineOpen, candidate, pending, complete, changed, viewRef]);
  useEffect(() => () => controller.current?.abort(), []);

  const extensions = useMemo(
    () => [
      writingPromptExtension(promptActions),
      writingReviewExtension(reviewActions),
      EditorView.updateListener.of((update) => {
        if (update.selectionSet || update.docChanged) {
          const { from, to } = update.state.selection.main;
          setSelection(update.state.doc.sliceString(from, to));
          requestAnimationFrame(() => {
            if (!update.view.dom.isConnected) return;
            const coordinates = update.view.coordsAtPos(to);
            setSelectionAnchor(
              coordinates && from !== to
                ? { left: coordinates.left, top: coordinates.bottom + 8 }
                : null,
            );
          });
        }
        if (update.docChanged) {
          insertionPoint.current = update.changes.mapPos(insertionPoint.current, 1);
          commands.current.documentChanged();
        }
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
          commands.current.open("draft");
          return true;
        },
        scroll() {
          setSelectionAnchor(null);
          return false;
        },
      }),
    ],
    [],
  );
  return {
    extensions,
    selection,
    selectionAnchor,
    action,
    origin,
    instruction,
    setInstruction: updateInstruction,
    candidate,
    pending,
    complete,
    error,
    changed,
    messages,
    panelOpen,
    setPanelOpen,
    mode,
    setMode,
    inlineOpen,
    open,
    close: dismissReview,
    generate,
    accept,
    insertReply,
    stop,
    newConversation,
  };
}
