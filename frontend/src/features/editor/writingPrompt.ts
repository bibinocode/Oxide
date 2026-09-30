import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";
import { observeComposer, positionComposerMenu, resizeComposer } from "./composerSize";
import type { WritingImage } from "./hooks/useWritingAssistant";
import { composerIcon } from "./composerIcon";
import aiChatGif from "../../assets/ai-caht.gif";

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
  hide: () => void;
  stop: () => void;
  search: () => boolean;
  searchAvailable: () => boolean;
  toggleSearch: () => void;
  openPanel: () => void;
  snapshot: () => {
    instruction: string;
    images: WritingImage[];
    reading: boolean;
    pending: boolean;
    imageSupported: boolean;
    error: string;
  };
  attachImages: (files: File[]) => Promise<void>;
  removeImage: (index: number) => void;
  subscribe: (listener: () => void) => () => void;
}

class PromptWidget extends WidgetType {
  private dispose?: () => void;
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
    const avatar = document.createElement("img");
    avatar.className = "writing-ai-avatar";
    avatar.src = aiChatGif;
    avatar.alt = "";
    badge.append(avatar);
    badge.setAttribute("aria-hidden", "true");
    if (this.pending) {
      root.classList.add("writing-generating-prompt");
      const status = document.createElement("span");
      status.className = "writing-prompt-status writing-thinking-text";
      status.setAttribute("role", "status");
      status.textContent = "正在工作";
      const stop = document.createElement("button");
      stop.type = "button";
      stop.className = "writing-composer-icon";
      stop.append(composerIcon("stop"));
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
    input.addEventListener("input", () => {
      this.actions.current.input(input.value);
      send.disabled =
        this.actions.current.snapshot().reading ||
        (!input.value.trim() && !this.actions.current.snapshot().images.length);
      resizeComposer(input);
      view.requestMeasure();
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
        event.preventDefault();
        this.actions.current.generate();
      }
      if (event.key === "Escape" && !tools.open) {
        event.preventDefault();
        this.actions.current.close();
        view.focus();
      }
    });
    const send = document.createElement("button");
    send.type = "button";
    send.className = "writing-send";
    send.disabled = true;
    send.append(composerIcon("arrow"));
    send.setAttribute("aria-label", "发送给 AI");
    send.addEventListener("click", () => this.actions.current.generate());
    const close = document.createElement("button");
    close.type = "button";
    close.className = "writing-composer-icon";
    close.append(composerIcon("close"), document.createTextNode("关闭 AI 输入框"));
    close.setAttribute("aria-label", "关闭 AI 输入框");
    close.addEventListener("click", () => {
      this.actions.current.close();
      view.focus();
    });
    const tools = document.createElement("details");
    tools.className = "writing-composer-menu";
    const trigger = document.createElement("summary");
    trigger.className = "writing-composer-icon";
    trigger.classList.add("writing-wand");
    const triggerAvatar = avatar.cloneNode(true);
    trigger.append(triggerAvatar);
    trigger.setAttribute("aria-label", "AI 输入工具");
    const menu = document.createElement("div");
    menu.className = "writing-composer-options";
    menu.setAttribute("popover", "manual");
    tools.addEventListener("toggle", () => positionComposerMenu(tools));
    const searchLabel = document.createElement("label");
    searchLabel.className = "writing-search-toggle";
    const search = document.createElement("input");
    search.type = "checkbox";
    const searchText = document.createElement("span");
    searchText.className = "writing-search-label";
    searchText.textContent = "联网搜索";
    searchLabel.append(search, composerIcon("globe"), searchText);
    search.setAttribute("aria-label", "联网搜索");
    search.addEventListener("change", () => {
      this.actions.current.toggleSearch();
    });
    const fileInput = document.createElement("input");
    fileInput.type = "file";
    fileInput.accept = "image/png,image/jpeg,image/gif,image/webp";
    fileInput.multiple = true;
    fileInput.hidden = true;
    fileInput.setAttribute("aria-label", "AI 图片附件");
    fileInput.addEventListener("change", () => {
      void this.actions.current.attachImages(Array.from(fileInput.files ?? []));
      fileInput.value = "";
    });
    const attachments = document.createElement("button");
    attachments.type = "button";
    attachments.className = "writing-composer-icon writing-attach";
    attachments.append(composerIcon("clip"));
    attachments.setAttribute("aria-label", "添加图片");
    attachments.addEventListener("click", () => {
      tools.open = false;
      fileInput.click();
    });
    const openPanel = document.createElement("button");
    openPanel.type = "button";
    openPanel.textContent = "打开 AI 对话";
    openPanel.addEventListener("click", () => {
      tools.open = false;
      this.actions.current.openPanel();
    });
    menu.append(openPanel, close);
    tools.append(trigger, menu);
    const footer = document.createElement("footer");
    footer.className = "writing-composer-footer";
    const controls = document.createElement("div");
    controls.className = "writing-composer-controls";
    controls.append(attachments, send);
    footer.append(searchLabel, controls);
    const previews = document.createElement("div");
    previews.className = "writing-image-list";
    const error = document.createElement("p");
    error.className = "writing-composer-error";
    error.setAttribute("role", "alert");
    root.append(fileInput, previews, tools, input, footer, error);
    let displayedImages: WritingImage[] | undefined;
    const refresh = () => {
      const state = this.actions.current.snapshot();
      root.dataset.expanded = String(!!state.instruction || !!state.images.length || !!state.error);
      if (input.value !== state.instruction) {
        input.value = state.instruction;
        resizeComposer(input);
      }
      search.disabled = state.pending || !this.actions.current.searchAvailable();
      search.checked = this.actions.current.search() && this.actions.current.searchAvailable();
      searchLabel.title = this.actions.current.searchAvailable()
        ? "允许 AI 为本次消息联网搜索并读取公开文章链接"
        : "请在 Agent 工具配置中启用并授权写作";
      attachments.disabled =
        state.pending || state.reading || !state.imageSupported || state.images.length >= 3;
      attachments.setAttribute("aria-label", state.reading ? "读取图片中" : "添加图片");
      attachments.title = state.imageSupported
        ? "最多 3 张，每张不超过 2 MiB"
        : "当前写作模型不支持图片理解";
      send.disabled =
        state.pending || state.reading || (!state.instruction.trim() && !state.images.length);
      error.textContent = state.error;
      error.hidden = !state.error;
      previews.hidden = !state.images.length;
      if (displayedImages !== state.images) {
        displayedImages = state.images;
        previews.replaceChildren(
          ...state.images.map((image, index) => {
            const item = document.createElement("div");
            const thumbnail = document.createElement("img");
            thumbnail.src = "data:" + image.mime_type + ";base64," + image.data;
            thumbnail.alt = image.name;
            const remove = document.createElement("button");
            remove.type = "button";
            remove.append(composerIcon("close"));
            remove.setAttribute("aria-label", "移除图片 " + image.name);
            remove.addEventListener("click", () => this.actions.current.removeImage(index));
            item.append(thumbnail, remove);
            return item;
          }),
        );
      }
      view.requestMeasure();
    };
    refresh();
    const unsubscribe = this.actions.current.subscribe(refresh);
    const dismiss = (event: Event) => {
      if (root.contains(event.target as Node)) return;
      tools.open = false;
      if ((event.target as HTMLElement).closest("[data-writing-panel-trigger]")) return;
      this.actions.current.hide();
    };
    root.addEventListener("focusout", () => {
      requestAnimationFrame(() => {
        if (
          root.isConnected &&
          !root.contains(document.activeElement) &&
          !document.activeElement?.closest("[data-writing-panel-trigger]")
        )
          this.actions.current.hide();
      });
    });
    root.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && tools.open) {
        event.stopPropagation();
        tools.open = false;
        trigger.focus();
      }
    });
    document.addEventListener("pointerdown", dismiss);
    const disconnect = observeComposer(input, () => view.requestMeasure());
    this.dispose = () => {
      unsubscribe();
      disconnect();
      document.removeEventListener("pointerdown", dismiss);
    };
    requestAnimationFrame(() => {
      if (input.isConnected) {
        input.focus();
        resizeComposer(input);
        view.requestMeasure();
      }
    });
    return root;
  }
  ignoreEvent() {
    return true;
  }
  destroy() {
    this.dispose?.();
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
