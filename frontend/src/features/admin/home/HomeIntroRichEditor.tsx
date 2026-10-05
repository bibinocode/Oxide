import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { Bold, Italic, Link2, Link2Off, List, ListOrdered, Undo2, Redo2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { parseProfile } from "../../contact/profile";

/** 首页介绍保留 Markdown 存储格式，管理端提供所见即所得编辑。 */
export function HomeIntroRichEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkLabel, setLinkLabel] = useState("");
  const [linkUrl, setLinkUrl] = useState("");
  const [linkError, setLinkError] = useState("");
  const editor = useEditor({
    immediatelyRender: false,
    extensions: [
      StarterKit.configure({
        heading: false,
        codeBlock: false,
        horizontalRule: false,
        link: { openOnClick: false, autolink: false },
      }),
      Markdown,
    ],
    content: value,
    contentType: "markdown",
    onUpdate: ({ editor: instance }) => onChangeRef.current(instance.getMarkdown()),
  });

  useEffect(() => {
    if (editor && editor.getMarkdown().trim() !== value.trim()) {
      editor.commands.setContent(value, { contentType: "markdown", emitUpdate: false });
    }
  }, [editor, value]);

  function openLink() {
    if (!editor) return;
    setLinkLabel(
      editor.state.doc.textBetween(editor.state.selection.from, editor.state.selection.to),
    );
    setLinkUrl(editor.getAttributes("link").href ?? "");
    setLinkError("");
    setLinkOpen(true);
  }

  function saveLink() {
    if (!editor) return;
    const raw = linkUrl.trim();
    const normalized = parseProfile(raw)?.url ?? raw;
    const safe =
      (normalized.startsWith("/") && !normalized.startsWith("//")) ||
      (normalized.startsWith("mailto:") && normalized.length > 7) ||
      (() => {
        try {
          return ["https:", "http:"].includes(new URL(normalized).protocol);
        } catch {
          return false;
        }
      })();
    if (!safe) {
      setLinkError("请输入 HTTPS、HTTP、mailto 或站内地址");
      return;
    }
    if (editor.isActive("link")) {
      editor.chain().focus().extendMarkRange("link").setLink({ href: normalized }).run();
    } else if (editor.state.selection.empty) {
      if (!linkLabel.trim()) {
        setLinkError("请输入链接文字");
        return;
      }
      editor
        .chain()
        .focus()
        .insertContent({
          type: "text",
          text: linkLabel.trim(),
          marks: [{ type: "link", attrs: { href: normalized } }],
        })
        .run();
    } else {
      editor.chain().focus().setLink({ href: normalized }).run();
    }
    setLinkOpen(false);
  }

  return (
    <div className="home-intro-rich-editor">
      <div className="home-intro-rich-toolbar" role="toolbar" aria-label="介绍格式">
        <button
          type="button"
          title="加粗"
          aria-label="加粗"
          aria-pressed={editor?.isActive("bold") ?? false}
          onClick={() => editor?.chain().focus().toggleBold().run()}
        >
          <Bold size={16} />
        </button>
        <button
          type="button"
          title="斜体"
          aria-label="斜体"
          aria-pressed={editor?.isActive("italic") ?? false}
          onClick={() => editor?.chain().focus().toggleItalic().run()}
        >
          <Italic size={16} />
        </button>
        <span className="home-intro-rich-toolbar-divider" />
        <button
          type="button"
          aria-label="无序列表"
          title="无序列表"
          aria-pressed={editor?.isActive("bulletList") ?? false}
          onClick={() => editor?.chain().focus().toggleBulletList().run()}
        >
          <List size={16} />
        </button>
        <button
          type="button"
          aria-label="有序列表"
          title="有序列表"
          aria-pressed={editor?.isActive("orderedList") ?? false}
          onClick={() => editor?.chain().focus().toggleOrderedList().run()}
        >
          <ListOrdered size={16} />
        </button>
        <button
          type="button"
          title="添加链接"
          aria-label="添加链接"
          aria-pressed={linkOpen}
          onClick={openLink}
        >
          <Link2 size={16} />
        </button>
        <button
          type="button"
          title="移除链接"
          aria-label="移除链接"
          disabled={!editor?.isActive("link")}
          onClick={() => editor?.chain().focus().unsetLink().run()}
        >
          <Link2Off size={16} />
        </button>
        <button
          type="button"
          aria-label="撤销"
          title="撤销"
          disabled={!editor?.can().undo()}
          onClick={() => editor?.chain().focus().undo().run()}
        >
          <Undo2 size={16} />
        </button>
        <button
          type="button"
          aria-label="重做"
          title="重做"
          disabled={!editor?.can().redo()}
          onClick={() => editor?.chain().focus().redo().run()}
        >
          <Redo2 size={16} />
        </button>
      </div>
      <EditorContent editor={editor} aria-label="介绍正文" />
      {linkOpen && (
        <div className="home-intro-link-form">
          {editor?.state.selection.empty && (
            <input
              className="field"
              aria-label="链接文字"
              placeholder="链接文字"
              value={linkLabel}
              onChange={(event) => setLinkLabel(event.target.value)}
            />
          )}
          <input
            className="field"
            aria-label="链接地址"
            type="text"
            placeholder="https://example.com"
            value={linkUrl}
            onChange={(event) => setLinkUrl(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                saveLink();
              }
            }}
          />
          <div className="flex gap-2">
            <button type="button" className="button-primary" onClick={saveLink}>
              插入链接
            </button>
            <button type="button" className="button-secondary" onClick={() => setLinkOpen(false)}>
              取消
            </button>
          </div>
          {linkError && (
            <p role="alert" className="text-sm text-warm">
              {linkError}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
