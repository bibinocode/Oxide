import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";
import { diffWordsWithSpace } from "diff";
import type { WritingAction } from "./hooks/useWritingAssistant";

/** 仅存编辑器装饰信息；候选不会进入 Markdown 文档或撤销栈。 */
export interface WritingReview {
  from: number;
  to: number;
  original: string;
  candidate: string;
  action: WritingAction;
  pending: boolean;
  complete: boolean;
  changed: boolean;
}

export interface WritingReviewActions {
  accept: () => void;
  close: () => void;
}

export const setWritingReview = StateEffect.define<WritingReview | null>();

/** 在选区末尾绘制原位候选，采纳前不改变编辑器正文。 */
class ReviewWidget extends WidgetType {
  constructor(
    readonly review: WritingReview,
    readonly actions: { current: WritingReviewActions },
  ) {
    super();
  }

  eq(other: ReviewWidget) {
    return JSON.stringify(this.review) === JSON.stringify(other.review);
  }

  toDOM() {
    const root = document.createElement("div");
    root.className = "writing-inline-review";
    root.setAttribute("aria-label", "原位 AI 写作差异");
    const heading = document.createElement("div");
    heading.className = "writing-inline-heading";
    heading.textContent = this.review.pending
      ? "AI 正在写作…"
      : this.review.complete
        ? "AI 候选"
        : "AI 写作";
    root.append(heading);

    const diff = document.createElement("div");
    diff.className = "writing-inline-diff";
    const original = this.review.action === "improve" ? this.review.original : "";
    for (const part of diffWordsWithSpace(original, this.review.candidate)) {
      const span = document.createElement("span");
      span.textContent = part.value;
      if (part.added) span.className = "writing-diff-added";
      if (part.removed) span.className = "writing-diff-removed";
      diff.append(span);
    }
    if (!this.review.candidate) diff.textContent = "等待模型输出…";
    root.append(diff);

    const actions = document.createElement("div");
    actions.className = "writing-inline-actions";
    if (this.review.action !== "explain") {
      const accept = document.createElement("button");
      accept.type = "button";
      accept.textContent = "采纳";
      accept.disabled =
        !this.review.complete || this.review.changed || !this.review.candidate.trim();
      accept.addEventListener("click", () => this.actions.current.accept());
      actions.append(accept);
    }
    const dismiss = document.createElement("button");
    dismiss.type = "button";
    dismiss.textContent = "放弃";
    dismiss.addEventListener("click", () => this.actions.current.close());
    actions.append(dismiss);
    root.append(actions);
    return root;
  }
}

/** 文档变更即清理旧装饰；Hook 随 SSE 增量推送新的候选快照。 */
export function writingReviewExtension(actions: { current: WritingReviewActions }) {
  return StateField.define<WritingReview | null>({
    create: () => null,
    update(value, transaction) {
      if (transaction.docChanged) value = null;
      for (const effect of transaction.effects) {
        if (effect.is(setWritingReview)) value = effect.value;
      }
      return value;
    },
    provide: (field) =>
      EditorView.decorations.from(field, (review) => {
        if (!review) return Decoration.none;
        const marks = [];
        if (review.action === "improve" && review.from < review.to) {
          marks.push(
            Decoration.mark({ class: "writing-original-removed" }).range(review.from, review.to),
          );
        }
        marks.push(
          Decoration.widget({
            widget: new ReviewWidget(review, actions),
            block: true,
            side: 1,
          }).range(review.to),
        );
        return Decoration.set(marks, true);
      }),
  });
}
