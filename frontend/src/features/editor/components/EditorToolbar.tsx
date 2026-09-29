import {
  Bold,
  Code2,
  Heading2,
  ImagePlus,
  Italic,
  Link2,
  List,
  ListOrdered,
  Minus,
  Quote,
  Strikethrough,
} from "lucide-react";

export type MarkdownCommand =
  | "heading"
  | "bold"
  | "italic"
  | "strike"
  | "quote"
  | "link"
  | "image"
  | "code"
  | "bullet"
  | "ordered"
  | "rule";

/** Markdown 工具栏只插入源码语法，始终保持左侧为可编辑原文。 */
export function EditorToolbar({ onCommand }: { onCommand: (command: MarkdownCommand) => void }) {
  const items = [
    { name: "标题", icon: Heading2, command: "heading" },
    { name: "加粗", icon: Bold, command: "bold" },
    { name: "斜体", icon: Italic, command: "italic" },
    { name: "删除线", icon: Strikethrough, command: "strike" },
    { name: "引用", icon: Quote, command: "quote" },
    { name: "网页链接", icon: Link2, command: "link" },
    { name: "图片", icon: ImagePlus, command: "image" },
    { name: "代码块", icon: Code2, command: "code" },
    { name: "项目列表", icon: List, command: "bullet" },
    { name: "编号列表", icon: ListOrdered, command: "ordered" },
    { name: "分隔线", icon: Minus, command: "rule" },
  ] as const;

  return (
    <div className="markdown-toolbar" role="toolbar" aria-label="Markdown 格式">
      {items.map((item) => (
        <button
          key={item.command}
          type="button"
          title={item.name}
          aria-label={item.name}
          onClick={() => onCommand(item.command)}
        >
          <item.icon size={17} />
        </button>
      ))}
    </div>
  );
}
