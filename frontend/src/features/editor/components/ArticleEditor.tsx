import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { EditorView } from "@codemirror/view";
import { ArrowLeft, History, Save, Trash2 } from "lucide-react";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";
import { ArticlePresentation } from "../../article/components/ArticlePresentation";
import { apiRequest } from "../../../lib/api/client";
import type { AdminArticle, Taxonomy } from "../../../lib/api/types";
import { sourceFromDocument, type MarkdownDocument } from "../markdownDocument";
import { EditorToolbar, type MarkdownCommand } from "./EditorToolbar";
import { PublishPanel, type CoverSelection } from "./PublishPanel";
import { useArticlePreview } from "../hooks/useArticlePreview";
import { htmlToMarkdown } from "../htmlToMarkdown";
import { ThemeControl } from "../../../components/layout/ThemeControl";

const editorExtensions = [
  markdown(),
  EditorView.lineWrapping,
  EditorView.domEventHandlers({
    paste(event, view) {
      const html = event.clipboardData?.getData("text/html");
      if (!html) return false;
      const converted = htmlToMarkdown(html);
      if (!converted) return false;
      event.preventDefault();
      const { from, to } = view.state.selection.main;
      view.dispatch({
        changes: { from, to, insert: converted },
        selection: { anchor: from + converted.length },
      });
      return true;
    },
  }),
];

/** Markdown 工作区：源码和预览共同占满可用空间，文章属性独立收纳。 */
export function ArticleEditor({ publicId }: { publicId?: string }) {
  const { session } = useAdminSession();
  const navigate = useNavigate();
  const editorView = useRef<EditorView | null>(null);
  const imageInput = useRef<HTMLInputElement>(null);
  const [title, setTitle] = useState("");
  const [slug, setSlug] = useState("");
  const [summary, setSummary] = useState("");
  const [source, setSource] = useState("");
  const [status, setStatus] = useState<"draft" | "published">("draft");
  const [publishedAt, setPublishedAt] = useState<string | null>(null);
  const [view, setView] = useState<"edit" | "preview">("edit");
  const [publishOpen, setPublishOpen] = useState(false);
  const [cover, setCover] = useState<CoverSelection>({ asset_public_id: null, media_url: null });
  const savedId = useRef(publicId);
  const draftSlug = useRef("");
  const [message, setMessage] = useState("");
  const [pending, setPending] = useState(false);
  const [categories, setCategories] = useState<Taxonomy[]>([]);
  const [tags, setTags] = useState<Taxonomy[]>([]);
  const [selectedCategories, setSelectedCategories] = useState<string[]>([]);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [revisions, setRevisions] = useState<
    { saved_at: string; document: Record<string, unknown> }[]
  >([]);
  const { html: previewHtml, error: previewError } = useArticlePreview(source, session);
  const lines = source ? source.split("\n").length : 1;

  useEffect(() => {
    Promise.all([
      apiRequest<Taxonomy[]>("/api/v1/categories"),
      apiRequest<Taxonomy[]>("/api/v1/tags"),
    ])
      .then(([categoryItems, tagItems]) => {
        setCategories(categoryItems);
        setTags(tagItems);
      })
      .catch((error) => setMessage(error.message));
  }, []);

  useEffect(() => {
    if (!publicId) return;
    savedId.current = publicId;
    apiRequest<CoverSelection>(`/api/v1/admin/articles/${publicId}/cover`)
      .then(setCover)
      .catch((error) => setMessage(error.message));
    apiRequest<AdminArticle>(`/api/v1/admin/articles/${publicId}`)
      .then((article) => {
        setTitle(article.title);
        setSlug(article.slug);
        setSummary(article.summary ?? "");
        setStatus(article.status);
        setPublishedAt(article.published_at);
        setSource(sourceFromDocument(article.document));
        if (article.document.type !== "markdown") {
          setMessage("旧文章已转换为 Markdown；保存后采用新格式");
        }
      })
      .catch((error) => setMessage(error.message));
    apiRequest<{ categories: string[]; tags: string[] }>(
      `/api/v1/admin/articles/${publicId}/taxonomy`,
    )
      .then((data) => {
        setSelectedCategories(data.categories);
        setSelectedTags(data.tags);
      })
      .catch((error) => setMessage(error.message));
    apiRequest<{ saved_at: string; document: Record<string, unknown> }[]>(
      `/api/v1/admin/articles/${publicId}/revisions`,
    )
      .then(setRevisions)
      .catch((error) => setMessage(error.message));
  }, [publicId]);

  /** 在当前选区插入 Markdown 语法，保留光标位置供连续写作。 */
  function insert(before: string, after = "", placeholder = "文字") {
    const view = editorView.current;
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const selected = view.state.doc.sliceString(from, to) || placeholder;
    view.dispatch({
      changes: { from, to, insert: before + selected + after },
      selection: { anchor: from + before.length, head: from + before.length + selected.length },
    });
    view.focus();
  }

  function format(command: MarkdownCommand) {
    switch (command) {
      case "heading":
        insert("## ", "", "小标题");
        break;
      case "bold":
        insert("**", "**");
        break;
      case "italic":
        insert("*", "*");
        break;
      case "strike":
        insert("~~", "~~");
        break;
      case "quote":
        insert("> ");
        break;
      case "link":
        insert("[", "](https://example.com)", "网页标题");
        break;
      case "image":
        imageInput.current?.click();
        break;
      case "code":
        insert("\n```rust\n", "\n```\n", "代码");
        break;
      case "bullet":
        insert("- ", "", "列表项");
        break;
      case "ordered":
        insert("1. ", "", "列表项");
        break;
      case "rule":
        insert("\n\n", "\n\n", "---");
        break;
    }
  }

  /** 保存 Markdown 原文；服务端负责生成正式公开 HTML。 */
  async function persistArticle(): Promise<AdminArticle> {
    if (!session) throw new Error("请先登录");
    const document: MarkdownDocument = { type: "markdown", source };
    const articleSlug = ensureSlug();
    const article = await apiRequest<AdminArticle>(
      savedId.current ? `/api/v1/admin/articles/${savedId.current}` : "/api/v1/admin/articles",
      {
        method: savedId.current ? "PUT" : "POST",
        headers: csrfHeaders(session),
        body: JSON.stringify({
          title: title.trim() || "未命名文章",
          slug: articleSlug,
          summary: summary || null,
          document,
        }),
      },
    );
    savedId.current = article.public_id;
    await apiRequest(`/api/v1/admin/articles/${article.public_id}/cover`, {
      method: "PUT",
      headers: csrfHeaders(session),
      body: JSON.stringify({ asset_public_id: cover.asset_public_id }),
    });
    await apiRequest(`/api/v1/admin/articles/${article.public_id}/taxonomy`, {
      method: "PUT",
      headers: csrfHeaders(session),
      body: JSON.stringify({ categories: selectedCategories, tags: selectedTags }),
    });
    return article;
  }

  async function save() {
    setPending(true);
    setMessage("");
    try {
      const article = await persistArticle();
      setMessage("已保存");
      if (!publicId) {
        await navigate({
          to: "/admin/articles/$publicId",
          params: { publicId: article.public_id },
        });
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "保存失败");
    } finally {
      setPending(false);
    }
  }

  async function changePublication() {
    if (!session) return;
    setPending(true);
    setMessage("");
    try {
      const saved = await persistArticle();
      const action = "publish";
      const article = await apiRequest<AdminArticle>(
        `/api/v1/admin/articles/${saved.public_id}/${action}`,
        { method: "POST", headers: csrfHeaders(session) },
      );
      setStatus(article.status);
      setPublishedAt(article.published_at);
      setPublishOpen(false);
      setMessage(article.status === "published" ? "文章已发布" : "文章已撤回");
      if (!publicId) {
        await navigate({
          to: "/admin/articles/$publicId",
          params: { publicId: article.public_id },
        });
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "操作失败");
    } finally {
      setPending(false);
    }
  }

  async function removeArticle() {
    if (!publicId || !session || !window.confirm("永久删除这篇文章及其评论？")) return;
    try {
      await apiRequest(`/api/v1/admin/articles/${publicId}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      await navigate({ to: "/admin" });
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
    }
  }

  /** 撤回只改变公开状态，保留当前尚未保存的编辑内容。 */
  async function withdrawArticle() {
    if (!publicId || !session || pending) return;
    setPending(true);
    try {
      const article = await apiRequest<AdminArticle>(
        `/api/v1/admin/articles/${publicId}/unpublish`,
        {
          method: "POST",
          headers: csrfHeaders(session),
        },
      );
      setStatus(article.status);
      setMessage("文章已撤回草稿");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "撤回失败");
    } finally {
      setPending(false);
    }
  }

  async function uploadCover(file: File): Promise<CoverSelection> {
    if (!session) throw new Error("请先登录");
    const body = new FormData();
    body.append("file", file);
    try {
      const asset = await apiRequest<{ public_id: string; media_url: string }>(
        "/api/v1/admin/assets",
        { method: "POST", headers: csrfHeaders(session), body },
      );
      await apiRequest(`/api/v1/admin/assets/${asset.public_id}`, {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ public: true }),
      });
      return { asset_public_id: asset.public_id, media_url: asset.media_url };
    } catch (error) {
      throw new Error(error instanceof Error ? error.message : "上传失败");
    }
  }

  async function uploadImage(file: File) {
    try {
      const asset = await uploadCover(file);
      insert("![", `](${asset.media_url})`, file.name);
      setMessage("图片已上传");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "上传失败");
    }
  }

  /** 草稿无需填写发布属性，缺省链接只生成一次以避免重试产生重复文章。 */
  function ensureSlug() {
    if (slug) return slug;
    draftSlug.current ||= `article-${crypto.randomUUID().slice(0, 12)}`;
    setSlug(draftSlug.current);
    return draftSlug.current;
  }

  return (
    <section
      className="article-editor-shell markdown-editor"
      data-view={view}
      onKeyDown={(event) => {
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
          event.preventDefault();
          if (!pending) void save();
        }
      }}
    >
      <header className="markdown-commandbar">
        <Link to="/admin" title="返回文章列表" aria-label="返回文章列表" className="markdown-back">
          <ArrowLeft size={19} />
        </Link>
        <input
          className="markdown-title"
          aria-label="文章标题"
          placeholder="输入文章标题..."
          value={title}
          maxLength={160}
          onChange={(event) => setTitle(event.target.value)}
        />
        <span className="markdown-save-status" role="status">
          {message}
        </span>
        <ThemeControl compact />
        {publicId && (
          <details className="editor-history-menu">
            <summary title="修订历史" aria-label="修订历史">
              <History size={17} />
            </summary>
            <div>
              {revisions.map((revision) => (
                <button
                  key={revision.saved_at}
                  type="button"
                  onClick={() => {
                    setSource(sourceFromDocument(revision.document));
                    setMessage("已载入修订，保存后生效");
                  }}
                >
                  {new Date(revision.saved_at).toLocaleString("zh-CN")}
                </button>
              ))}
              {revisions.length === 0 && <p className="text-xs text-muted">暂无修订记录</p>}
              {status === "published" && (
                <button type="button" disabled={pending} onClick={withdrawArticle}>
                  撤回为草稿
                </button>
              )}
              <button type="button" onClick={removeArticle} className="text-warm">
                <Trash2 size={14} /> 删除文章
              </button>
            </div>
          </details>
        )}
        <button type="button" onClick={save} disabled={pending} className="button-secondary">
          <Save size={16} /> {status === "published" ? "保存修改" : "保存草稿"}
        </button>
        <button
          type="button"
          onClick={() => {
            ensureSlug();
            setMessage("");
            setPublishOpen(true);
          }}
          disabled={pending}
          className="button-primary"
        >
          {status === "draft" ? "发布" : "发布设置"}
        </button>
      </header>
      <div className="markdown-tool-row">
        <EditorToolbar onCommand={format} />
        <span className="markdown-format-label">MARKDOWN</span>
      </div>
      <input
        ref={imageInput}
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) void uploadImage(file);
          event.target.value = "";
        }}
      />
      <div className="editor-view-switch" aria-label="编辑视图">
        <button type="button" aria-pressed={view === "edit"} onClick={() => setView("edit")}>
          源码
        </button>
        <button type="button" aria-pressed={view === "preview"} onClick={() => setView("preview")}>
          预览
        </button>
      </div>
      <div className="editor-workspace">
        <div className="editor-pane editor-edit-pane markdown-source">
          <CodeMirror
            value={source}
            height="100%"
            extensions={editorExtensions}
            basicSetup={{
              lineNumbers: false,
              foldGutter: false,
              highlightActiveLine: false,
              highlightActiveLineGutter: false,
            }}
            onCreateEditor={(view) => {
              editorView.current = view;
            }}
            onChange={setSource}
            aria-label="Markdown 正文"
            placeholder="从这里开始写作..."
          />
        </div>
        <aside className="editor-pane editor-preview-pane" aria-label="实时预览">
          <div className="editor-preview-content">
            <div>
              <ArticlePresentation
                title={title}
                summary={summary || null}
                publishedAt={publishedAt}
                html={previewHtml}
                coverUrl={cover.media_url}
              />
            </div>
          </div>
        </aside>
      </div>
      <footer className="markdown-statusbar">
        <span>
          字符数 {source.length} · 行数 {lines}
        </span>
        <span role="status">{previewError || (status === "published" ? "已发布" : "草稿")}</span>
      </footer>
      {publishOpen && (
        <PublishPanel
          slug={slug}
          summary={summary}
          cover={cover}
          categories={categories}
          tags={tags}
          selectedCategories={selectedCategories}
          selectedTags={selectedTags}
          pending={pending}
          message={message}
          published={status === "published"}
          onSlug={setSlug}
          onSummary={setSummary}
          onCover={setCover}
          onCategories={setSelectedCategories}
          onTags={setSelectedTags}
          onUpload={uploadCover}
          onPublish={changePublication}
          onClose={() => setPublishOpen(false)}
        />
      )}
    </section>
  );
}
