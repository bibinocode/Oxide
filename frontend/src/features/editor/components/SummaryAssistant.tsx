import { Sparkles } from "lucide-react";
import { useArticleSummary } from "../hooks/useArticleSummary";

/** 发布面板中的摘要候选，采纳前不会改动文章字段。 */
export function SummaryAssistant({
  title,
  source,
  onApply,
}: {
  title: string;
  source: string;
  onApply: (summary: string) => void;
}) {
  const { candidate, pending, error, generate, dismiss } = useArticleSummary(title, source);

  return (
    <div className="border-t border-line pt-4">
      <button
        type="button"
        className="button-secondary text-sm"
        disabled={pending || !title.trim() || !source.trim()}
        onClick={() => void generate()}
      >
        <Sparkles size={16} /> {pending ? "生成中…" : candidate ? "重新生成摘要" : "AI 生成摘要"}
      </button>
      {error && (
        <p role="alert" className="mt-3 text-sm text-warm">
          {error}
        </p>
      )}
      {candidate && (
        <div className="mt-4 border-l-2 border-accent bg-surface px-4 py-3">
          <p className="whitespace-pre-wrap text-sm leading-relaxed">{candidate}</p>
          <button
            type="button"
            className="mt-3 text-sm font-semibold text-accent hover:underline"
            onClick={() => {
              onApply(candidate);
              dismiss();
            }}
          >
            采用摘要
          </button>
        </div>
      )}
    </div>
  );
}
