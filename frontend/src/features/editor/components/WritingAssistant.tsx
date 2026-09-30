import { useEffect, useRef, useState, type CSSProperties } from "react";
import { SidebarResizeHandle } from "./SidebarResizeHandle";
import { WritingMarkdown } from "./WritingMarkdown";
import {
  ArrowUp,
  Check,
  Copy,
  Maximize2,
  MessageSquarePlus,
  PanelRight,
  Plus,
  Sparkles,
  Square,
  X,
  PictureInPicture2,
  ChevronDown,
} from "lucide-react";
import type { ConversationMode, useWritingAssistant } from "../hooks/useWritingAssistant";

type Assistant = ReturnType<typeof useWritingAssistant>;
const actionLabels = { explain: "问 AI", improve: "提升写作", continue: "续写" };
const modes: { value: ConversationMode; label: string; icon: typeof PanelRight }[] = [
  { value: "sidebar", label: "侧边栏", icon: PanelRight },
  { value: "floating", label: "浮动", icon: PictureInPicture2 },
  { value: "fullscreen", label: "全屏", icon: Maximize2 },
];

/** 选区操作跟随光标坐标，鼠标按下不会清除 CodeMirror 中的原选区。 */
export function WritingSelectionActions({ assistant }: { assistant: Assistant }) {
  if (
    !assistant.selection.trim() ||
    !assistant.selectionAnchor ||
    assistant.inlineOpen ||
    assistant.action === "improve" ||
    assistant.action === "continue" ||
    assistant.pending
  )
    return null;
  const { left, top } = assistant.selectionAnchor;
  return (
    <div
      className="writing-selection-actions"
      aria-label="选中文字的 AI 操作"
      style={{
        left: Math.max(12, Math.min(left, window.innerWidth - 280)),
        top: Math.min(top, window.innerHeight - 52),
      }}
      onMouseDown={(event) => event.preventDefault()}
    >
      <Sparkles size={15} aria-hidden="true" />
      {(["explain", "improve", "continue"] as const).map((action) => (
        <button key={action} type="button" onClick={() => assistant.open(action)}>
          {actionLabels[action]}
        </button>
      ))}
    </div>
  );
}

/** 独立对话面板支持连续追问；显示模式切换只改变布局，不重建会话。 */
export function WritingAssistant({ assistant }: { assistant: Assistant }) {
  const panel = useRef<HTMLElement>(null);
  const [sidebarWidth, setSidebarWidth] = useState(400);
  const input = useRef<HTMLTextAreaElement>(null);
  const conversation = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const [copied, setCopied] = useState<number | null>(null);
  const [notice, setNotice] = useState("");
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  useEffect(() => {
    try {
      const stored = Number(localStorage.getItem("oxide.editor.sidebar-width"));
      if (stored >= 320 && Number.isFinite(stored)) setSidebarWidth(Math.min(800, stored));
    } catch {
      /* 禁用本地存储时保留默认宽度。 */
    }
  }, []);
  function resizeSidebar(width: number) {
    setSidebarWidth(width);
    try {
      localStorage.setItem("oxide.editor.sidebar-width", String(width));
    } catch {
      /* 布局偏好保存失败不影响拖动。 */
    }
  }
  useEffect(() => {
    if (assistant.panelOpen) input.current?.focus();
  }, [assistant.panelOpen]);
  useEffect(() => {
    if (follow.current && conversation.current)
      conversation.current.scrollTop = conversation.current.scrollHeight;
  }, [assistant.messages, assistant.panelOpen]);

  async function copy(id: number, content: string) {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(id);
      setNotice("回复已复制");
    } catch {
      setNotice("复制失败，请选择回复文本手动复制。");
    }
  }
  if (!assistant.panelOpen) return null;
  const currentMode = modes.find((item) => item.value === assistant.mode)!;
  return (
    <aside
      ref={panel}
      className="writing-chat"
      data-mode={assistant.mode}
      aria-label="AI 对话面板"
      style={
        {
          "--editor-chat-width": sidebarWidth + "px",
          ...(assistant.mode === "floating" && position
            ? { left: position.left, top: position.top, right: "auto", bottom: "auto" }
            : {}),
        } as CSSProperties
      }
      onKeyDown={(event) => {
        if (event.key === "Escape") assistant.setPanelOpen(false);
      }}
    >
      {assistant.mode === "sidebar" && (
        <SidebarResizeHandle panel={panel} value={sidebarWidth} onChange={resizeSidebar} />
      )}
      <header
        className="writing-chat-header"
        onPointerDown={(event) => {
          if (
            assistant.mode !== "floating" ||
            (event.target as HTMLElement).closest("button, summary, details")
          )
            return;
          const bounds = event.currentTarget.parentElement!.getBoundingClientRect();
          drag.current = { x: event.clientX, y: event.clientY, left: bounds.left, top: bounds.top };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          if (!drag.current || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
          const bounds = event.currentTarget.parentElement!.getBoundingClientRect();
          setPosition({
            left: Math.max(
              0,
              Math.min(
                window.innerWidth - bounds.width,
                drag.current.left + event.clientX - drag.current.x,
              ),
            ),
            top: Math.max(
              0,
              Math.min(
                window.innerHeight - bounds.height,
                drag.current.top + event.clientY - drag.current.y,
              ),
            ),
          });
        }}
        onPointerUp={(event) => {
          drag.current = null;
          if (event.currentTarget.hasPointerCapture(event.pointerId))
            event.currentTarget.releasePointerCapture(event.pointerId);
        }}
      >
        <span className="writing-ai-badge">
          <Sparkles size={17} />
        </span>
        <span className="writing-chat-title">
          {assistant.messages.find((item) => item.role === "user")?.content || "AI 写作对话"}
        </span>
        <button
          type="button"
          onClick={assistant.newConversation}
          title="新建对话"
          aria-label="新建对话"
        >
          <MessageSquarePlus size={17} />
        </button>
        <details className="writing-mode-menu">
          <summary title="切换对话模式" aria-label="切换对话模式">
            <currentMode.icon size={17} />
            <ChevronDown size={12} />
          </summary>
          <div>
            {modes.map(({ value, label, icon: Icon }) => (
              <button
                key={value}
                type="button"
                aria-pressed={assistant.mode === value}
                onClick={(event) => {
                  assistant.setMode(value);
                  setPosition(null);
                  event.currentTarget.closest("details")?.removeAttribute("open");
                }}
              >
                <Icon size={16} />
                {label}
                {assistant.mode === value && <Check size={14} />}
              </button>
            ))}
          </div>
        </details>
        <button
          type="button"
          onClick={() => assistant.setPanelOpen(false)}
          title="收起对话（保留上下文）"
          aria-label="收起对话"
        >
          <X size={17} />
        </button>
      </header>
      <div
        className="writing-chat-messages"
        ref={conversation}
        onScroll={(event) => {
          const element = event.currentTarget;
          follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
        }}
      >
        {assistant.messages.length === 0 && (
          <div className="writing-chat-empty">
            <Sparkles size={24} />
            <p>从一个问题开始</p>
            <span>解释、构思或一起完善文章。回复不会自动修改正文。</span>
          </div>
        )}
        {assistant.messages.map((message) => (
          <article
            key={message.id}
            className="writing-chat-message"
            data-role={message.role}
            aria-label={message.role === "user" ? "我的问题" : "AI 回复"}
          >
            {message.role === "user" ? (
              <p className="writing-user-bubble">{message.content}</p>
            ) : (
              <>
                {message.progress.length > 0 && (
                  <details className="writing-progress">
                    <summary>{message.status === "streaming" ? "正在处理" : "处理过程"}</summary>
                    {message.progress.map((entry) => (
                      <div key={entry.kind + entry.id} data-kind={entry.kind}>
                        <span>
                          {entry.kind === "reasoning"
                            ? "思考过程"
                            : entry.kind === "tool"
                              ? "工具调用"
                              : "状态"}
                        </span>
                        <p>{entry.content}</p>
                      </div>
                    ))}
                  </details>
                )}
                {message.content && (
                  <div className="writing-chat-markdown">
                    <WritingMarkdown content={message.content} />
                  </div>
                )}
                {message.status === "streaming" && (
                  <div className="writing-generating" role="status">
                    <Sparkles size={14} />
                    {message.content ? "正在生成" : "正在准备"}
                  </div>
                )}
                {message.status === "stopped" && (
                  <p className="writing-message-status">已停止生成</p>
                )}
                {message.status === "error" && (
                  <p className="writing-message-status">本轮生成失败，可重新提问。</p>
                )}
                {message.status === "done" && message.content && (
                  <div className="writing-reply-actions">
                    <button
                      type="button"
                      onClick={() => void copy(message.id, message.content)}
                      title="复制回复"
                      aria-label="复制回复"
                    >
                      {copied === message.id ? <Check size={16} /> : <Copy size={16} />}
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        assistant.insertReply(message.content);
                        setNotice("已插入正文，可使用 Ctrl+Z 撤销");
                      }}
                      title="插入到此页面"
                      aria-label="插入到此页面"
                    >
                      <Plus size={17} />
                      <span>插入到此页面</span>
                    </button>
                  </div>
                )}
              </>
            )}
          </article>
        ))}
      </div>
      {assistant.changed && (assistant.action === "improve" || assistant.action === "continue") && (
        <p className="writing-assistant-error" role="alert">
          原文已变化，不能直接采纳旧候选；仍可复制或插入回复。
        </p>
      )}
      {assistant.error && (
        <p className="writing-assistant-error" role="alert">
          {assistant.error}
        </p>
      )}
      <div className="writing-chat-composer">
        <span className="writing-context-chip">
          <Sparkles size={12} /> 当前文章 ·{" "}
          {assistant.messages.filter((item) => item.role === "user").length} 轮对话
        </span>
        <textarea
          ref={input}
          value={assistant.instruction}
          onChange={(event) => assistant.setInstruction(event.target.value)}
          placeholder="使用 AI 处理各种任务…"
          aria-label="AI 对话输入"
          maxLength={1000}
          rows={3}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              follow.current = true;
              void assistant.generate();
            }
          }}
        />
        <footer>
          <span role="status">{notice || "Enter 发送 · Shift+Enter 换行"}</span>
          {assistant.pending ? (
            <button
              type="button"
              className="writing-send"
              onClick={assistant.stop}
              aria-label="停止生成"
            >
              <Square size={13} fill="currentColor" />
            </button>
          ) : (
            <button
              type="button"
              className="writing-send"
              disabled={!assistant.instruction.trim()}
              onClick={() => {
                follow.current = true;
                void assistant.generate();
              }}
              aria-label="发送消息"
            >
              <ArrowUp size={18} />
            </button>
          )}
        </footer>
      </div>
    </aside>
  );
}
