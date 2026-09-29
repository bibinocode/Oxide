/** 数据库中的 Markdown 文档格式；保留旧版 Tiptap JSON 的读取兼容。 */
export interface MarkdownDocument {
  type: "markdown";
  source: string;
}

interface LegacyNode {
  type?: string;
  text?: string;
  attrs?: Record<string, unknown>;
  marks?: { type?: string; attrs?: Record<string, unknown> }[];
  content?: LegacyNode[];
}

/** 打开历史文章时将受支持的 Tiptap 节点转成可继续编辑的 Markdown。 */
export function sourceFromDocument(document: Record<string, unknown>): string {
  if (document.type === "markdown" && typeof document.source === "string") {
    return document.source;
  }
  const root = document as LegacyNode;
  return (root.content ?? [])
    .map((node) => block(node))
    .join("\n\n")
    .trim();
}

/** 递归处理内联节点，将文本节点转换为 Markdown 格式 */
function inline(node: LegacyNode): string {
  if (node.type === "hardBreak") return "  \n";
  if (node.type !== "text") return (node.content ?? []).map(inline).join("");
  let value = (node.text ?? "").replace(/([\\`*_{}[\]()#+.!>|~-])/g, "\\$1");
  for (const mark of node.marks ?? []) {
    if (mark.type === "bold") value = `**${value}**`;
    if (mark.type === "italic") value = `*${value}*`;
    if (mark.type === "strike") value = `~~${value}~~`;
    if (mark.type === "code") value = `\`${node.text ?? ""}\``;
    if (mark.type === "link" && typeof mark.attrs?.href === "string") {
      value = `[${value}](${mark.attrs.href})`;
    }
  }
  return value;
}

/** 递归处理块级节点，将文本节点转换为 Markdown 格式 */
function block(node: LegacyNode): string {
  const content = node.content ?? [];
  switch (node.type) {
    case "paragraph":
      return content.map(inline).join("");
    case "heading":
      return `${"#".repeat(Number(node.attrs?.level) || 2)} ${content.map(inline).join("")}`;
    case "blockquote":
      return content
        .map(block)
        .join("\n\n")
        .split("\n")
        .map((line) => `> ${line}`)
        .join("\n");
    case "bulletList":
    case "orderedList":
      return content
        .map((item, index) => {
          const prefix = node.type === "orderedList" ? `${index + 1}. ` : "- ";
          return prefix + (item.content ?? []).map(block).join("\n\n").replaceAll("\n", "\n  ");
        })
        .join("\n");
    case "codeBlock":
      return `\`\`\`${typeof node.attrs?.language === "string" ? node.attrs.language : ""}\n${content.map((child) => child.text ?? "").join("")}\n\`\`\``;
    case "horizontalRule":
      return "---";
    case "image":
      return `![${String(node.attrs?.alt ?? "")}](${String(node.attrs?.src ?? "")})`;
    default:
      return content.map(block).join("\n\n");
  }
}
