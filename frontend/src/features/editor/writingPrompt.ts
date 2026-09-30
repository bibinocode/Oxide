import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";

/** 原位入口属于文档装饰，跟随唤起位置和编辑器滚动，不写入文章。 */
interface PromptPosition {
  position: number;
  label: string;
  pending?: boolean;
}
export const setWritingPrompt = StateEffect.define<PromptPosition | null>();
export interface WritingPromptActions {
  input: (text: string) => void;
  generate: () => void;
  close: () => void;
  stop: () => void;
}

class PromptWidget extends WidgetType {
  constructor(
    readonly label: string,
    readonly actions: { current: WritingPromptActions },
    readonly pending = false,
  ) {
    super();
  }
  eq(other: PromptWidget) {
    return other.label === this.label && other.pending === this.pending;
  }
  toDOM(view: EditorView) {
    const root = document.createElement("div");
    root.className = "writing-inline-prompt";
    const badge = document.createElement("span");
    badge.className = "writing-ai-badge";
    badge.textContent = "✦";
    badge.setAttribute("aria-hidden", "true");
    if (this.pending) {
      root.classList.add("writing-generating-prompt");
      const status = document.createElement("span");
      status.className = "writing-prompt-status";
      status.setAttribute("role", "status");
      status.textContent = "正在生成";
      const stop = document.createElement("button");
      stop.type = "button";
      stop.textContent = "■";
      stop.setAttribute("aria-label", "停止生成");
      stop.addEventListener("click", () => this.actions.current.stop());
      root.append(badge, status, stop);
      return root;
    }
    const input = document.createElement("textarea");
    input.placeholder = this.label;
    input.setAttribute("aria-label", "AI 写作要求");
    input.rows = 1;
    input.maxLength = 1000;
    input.addEventListener("input", () => this.actions.current.input(input.value));
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
        event.preventDefault();
        this.actions.current.generate();
      }
      if (event.key === "Escape") {
        event.preventDefault();
        this.actions.current.close();
        view.focus();
      }
    });
    const send = document.createElement("button");
    send.type = "button";
    send.textContent = "↑";
    send.setAttribute("aria-label", "发送给 AI");
    send.addEventListener("click", () => this.actions.current.generate());
    const close = document.createElement("button");
    close.type = "button";
    close.textContent = "×";
    close.setAttribute("aria-label", "关闭 AI 输入框");
    close.addEventListener("click", () => {
      this.actions.current.close();
      view.focus();
    });
    root.append(badge, input, send, close);
    requestAnimationFrame(() => {
      if (input.isConnected) {
        input.focus();
        view.requestMeasure();
      }
    });
    return root;
  }
  ignoreEvent() {
    return true;
  }
}

/** 块级装饰由 StateField 直接提供，保证 CodeMirror 正确计算布局高度。 */
export function writingPromptExtension(actions: { current: WritingPromptActions }) {
  return StateField.define<PromptPosition | null>({
    create: () => null,
    update(value, transaction) {
      if (value && transaction.docChanged)
        value = { ...value, position: transaction.changes.mapPos(value.position, 1) };
      for (const effect of transaction.effects)
        if (effect.is(setWritingPrompt)) value = effect.value;
      return value;
    },
    provide: (field) =>
      EditorView.decorations.from(field, (value) =>
        value
          ? Decoration.set([
              Decoration.widget({
                widget: new PromptWidget(value.label, actions, value.pending),
                block: true,
                side: 1,
              }).range(value.position),
            ])
          : Decoration.none,
      ),
  });
}
