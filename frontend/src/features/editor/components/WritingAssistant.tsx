import { useEffect, useRef } from "react";
import { ArrowUp, Sparkles, X } from "lucide-react";
import type { useWritingAssistant } from "../hooks/useWritingAssistant";

type Assistant = ReturnType<typeof useWritingAssistant>;

const actionLabels = {
  draft: "开始写作",
  explain: "解释",
  improve: "优化表达",
  continue: "续写",
};

/** 源码区内的写作入口和结果审阅面板。 */
export function WritingAssistant({ assistant }: { assistant: Assistant }) {
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (assistant.action) input.current?.focus();
  }, [assistant.action]);

  return (
    <>
      {assistant.selection.trim() && !assistant.action && (
        <div className="writing-selection-actions" aria-label="选中文字的 AI 操作">
          <Sparkles size={15} aria-hidden="true" />
          {(["explain", "improve", "continue"] as const).map((action) => (
            <button key={action} type="button" onClick={() => assistant.open(action)}>
              {actionLabels[action]}
            </button>
          ))}
        </div>
      )}
      {assistant.action && (
        <section className="writing-assistant" aria-label="AI 写作助手">
          <header>
            <span>
              <Sparkles size={15} /> {actionLabels[assistant.action]}
            </span>
            <button
              type="button"
              onClick={assistant.close}
              title="关闭写作助手"
              aria-label="关闭写作助手"
            >
              <X size={17} />
            </button>
          </header>
          <div className="writing-assistant-body">
            <div className="writing-prompt-row">
              <textarea
                ref={input}
                value={assistant.instruction}
                onChange={(event) => assistant.setInstruction(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                    event.preventDefault();
                    void assistant.generate();
                  }
                  if (event.key === "Escape") assistant.close();
                }}
                placeholder={assistant.action === "draft" ? "想写什么？" : "补充要求（可选）"}
                aria-label="AI 写作要求"
                maxLength={1000}
                rows={2}
              />
              <button
                type="button"
                className="button-primary"
                disabled={
                  assistant.pending ||
                  assistant.changed ||
                  (assistant.action === "draft" && !assistant.instruction.trim())
                }
                onClick={() => void assistant.generate()}
                title="生成内容"
                aria-label="生成内容"
              >
                <ArrowUp size={17} />
              </button>
            </div>
            {assistant.pending && <p role="status">正在原文位置生成候选…</p>}
            {assistant.error && (
              <p className="writing-assistant-error" role="alert">
                {assistant.error}
              </p>
            )}
            {assistant.changed && !assistant.candidate && (
              <p className="writing-assistant-error" role="alert">
                原文已变化，请重新选择后生成。
              </p>
            )}
            {assistant.candidate && assistant.complete && (
              <>
                <details className="writing-candidate-edit">
                  <summary>
                    {assistant.action === "explain" ? "查看完整解释" : "调整候选 Markdown"}
                  </summary>
                  <label htmlFor="writing-candidate">候选内容</label>
                  <textarea
                    id="writing-candidate"
                    className="writing-candidate"
                    value={assistant.candidate}
                    onChange={(event) => assistant.setCandidate(event.target.value)}
                    rows={8}
                    readOnly={assistant.action === "explain"}
                  />
                </details>
                {assistant.changed && (
                  <p className="writing-assistant-error" role="alert">
                    原文已变化，请关闭助手并重新选择后生成。
                  </p>
                )}
              </>
            )}
          </div>
        </section>
      )}
    </>
  );
}
