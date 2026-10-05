import Markdown from "react-markdown";
import { HomeIntroRichEditor } from "../home/HomeIntroRichEditor";

/** 关于页沿用介绍的富文本编辑器，实时预览使用公开页相同的 Markdown 排版。 */
export function AboutEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="home-intro-editor">
      <div className="home-intro-editor-fields">
        <p className="text-sm text-muted">独立展示在“关于我”页面，可编写经历、工作与联系方式。</p>
        <HomeIntroRichEditor value={value} onChange={onChange} />
        <p className="text-right text-xs text-muted">{Array.from(value).length} / 20000</p>
      </div>
      <div className="home-intro-editor-preview">
        <p className="eyebrow mb-8">关于页预览</p>
        <div className="prose-blog">
          <Markdown>{value || "关于我的内容正在整理中。"}</Markdown>
        </div>
      </div>
    </div>
  );
}
