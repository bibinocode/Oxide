import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { EditorView } from "@codemirror/view";
import { ArrowLeft, History, Save, Trash2, PanelRight, Sparkles } from "lucide-react";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";
import { ArticlePresentation } from "../../article/components/ArticlePresentation";
import { apiRequest } from "../../../lib/api/client";
import type { AdminArticle } from "../../../lib/api/types";
import { sourceFromDocument, type MarkdownDocument } from "../markdownDocument";
import { EditorToolbar, type MarkdownCommand } from "./EditorToolbar";
import { PublishPanel, type CoverSelection } from "./PublishPanel";
import { useArticlePreview } from "../hooks/useArticlePreview";
import { htmlToMarkdown } from "../htmlToMarkdown";
import { useWritingAssistant } from "../hooks/useWritingAssistant";
import { WritingAssistant, WritingSelectionActions } from "./WritingAssistant";
import { ResizeHandle } from "./ResizeHandle";
import { ThemeControl } from "../../../components/layout/ThemeControl";
import {
  editorDraftKey,
  readEditorDraft,
  storeEditorDraft,
  type EditorDraftValues,
} from "../editorDraft";
import { useEditorDraftProtection } from "../hooks/useEditorDraftProtection";
import { useEditorLayout } from "../hooks/useEditorLayout";
import { useArticleTaxonomies } from "../hooks/useArticleTaxonomies";

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
  const writing = useWritingAssistant(editorView);
  const { workspace, previewVisible, split, view, setPreviewVisible, setView, rememberLayout } =
    useEditorLayout();
  const imageInput = useRef<HTMLInputElement>(null);
  const [title, setTitle] = useState("");
  const [slug, setSlug] = useState("");
  const [summary, setSummary] = useState("");
  const [source, setSource] = useState("");
  const [status, setStatus] = useState<"draft" | "published">("draft");
  const [publishedAt, setPublishedAt] = useState<string | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);
  const [cover, setCover] = useState<CoverSelection>({ asset_public_id: null, media_url: null });
  const savedId = useRef(publicId);
  const draftSlug = useRef("");
  const [message, setMessage] = useState("");
  const [pending, setPending] = useState(false);
  const [ready, setReady] = useState(false);
  const [autosavePaused, setAutosavePaused] = useState(false);
  const [autosaveMessage, setAutosaveMessage] = useState("");
  const serverUpdatedAt = useRef<string | null>(null);
  const lastSaved = useRef("");
  const operation = useRef<Promise<AdminArticle> | null>(null);
  const { categories, tags, error: taxonomyError } = useArticleTaxonomies();
  const [selectedCategories, setSelectedCategories] = useState<string[]>([]);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [revisions, setRevisions] = useState<
    { saved_at: string; document: Record<string, unknown> }[]
  >([]);
  const { html: previewHtml, error: previewError } = useArticlePreview(source, session);
  const lines = source ? source.split("\n").length : 1;
  const values: EditorDraftValues = {
    title,
    slug,
    summary,
    source,
    cover,
    categories: selectedCategories,
    tags: selectedTags,
  };
  const latestValues = useRef(values);
  latestValues.current = values;
  const fingerprint = JSON.stringify(values);
  const storageKey = session ? editorDraftKey(session.username, publicId) : null;
  const dirty = ready && fingerprint !== lastSaved.current;
  const protection = useEditorDraftProtection({
    storageKey,
    ready,
    dirty,
    draft: {
      version: 1,
      values,
      savedId: savedId.current,
      serverUpdatedAt: serverUpdatedAt.current,
      savedAt: new Date().toISOString(),
    },
  });

  useEffect(() => {
    if (!session?.username) return;
    const active = new AbortController();
    const key = editorDraftKey(session.username, publicId);
    setReady(false);
    setAutosavePaused(false);
    setAutosaveMessage("");
    function applyValues(value: EditorDraftValues) {
      setTitle(value.title);
      setSlug(value.slug);
      setSummary(value.summary);
      setSource(value.source);
      setCover(value.cover);
      setSelectedCategories(value.categories);
      setSelectedTags(value.tags);
    }
    async function load() {
      let cached: ReturnType<typeof readEditorDraft> = null;
      try {
        cached = readEditorDraft(key);
      } catch (cause) {
        setMessage(cause instanceof Error ? cause.message : "本地草稿读取失败");
        setAutosavePaused(true);
      }
      const id = publicId ?? cached?.savedId;
      savedId.current = id;
      try {
        const empty: EditorDraftValues = {
          title: "",
          slug: "",
          summary: "",
          source: "",
          cover: { asset_public_id: null, media_url: null },
          categories: [],
          tags: [],
        };
        if (!id) {
          lastSaved.current = JSON.stringify(empty);
          serverUpdatedAt.current = null;
          applyValues(cached?.values ?? empty);
          setStatus("draft");
          setPublishedAt(null);
          setRevisions([]);
          if (cached) setAutosaveMessage("已恢复本地草稿");
        } else {
          const [article, currentCover, taxonomy, history] = await Promise.all([
            apiRequest<AdminArticle>("/api/v1/admin/articles/" + id, { signal: active.signal }),
            apiRequest<CoverSelection>("/api/v1/admin/articles/" + id + "/cover", {
              signal: active.signal,
            }),
            apiRequest<{ categories: string[]; tags: string[] }>(
              "/api/v1/admin/articles/" + id + "/taxonomy",
              { signal: active.signal },
            ),
            apiRequest<{ saved_at: string; document: Record<string, unknown> }[]>(
              "/api/v1/admin/articles/" + id + "/revisions",
              { signal: active.signal },
            ),
          ]);
          if (active.signal.aborted) return;
          const stored: EditorDraftValues = {
            title: article.title,
            slug: article.slug,
            summary: article.summary ?? "",
            source: sourceFromDocument(article.document),
            cover: currentCover,
            categories: taxonomy.categories,
            tags: taxonomy.tags,
          };
          lastSaved.current = JSON.stringify(stored);
          serverUpdatedAt.current = article.updated_at;
          setStatus(article.status);
          setPublishedAt(article.published_at);
          setRevisions(history);
          applyValues(cached?.values ?? stored);
          if (cached && JSON.stringify(cached.values) !== JSON.stringify(stored)) {
            const conflict =
              !!cached.serverUpdatedAt && cached.serverUpdatedAt !== article.updated_at;
            setAutosavePaused(conflict);
            setAutosaveMessage(
              conflict
                ? "已恢复本地草稿；服务器另有更新，自动保存已暂停，请核对后手动保存"
                : "已恢复尚未提交的本地草稿",
            );
          } else if (article.document.type !== "markdown")
            setMessage("旧文章已转换为 Markdown；保存后采用新格式");
        }
        if (!active.signal.aborted) setReady(true);
      } catch (cause) {
        if (active.signal.aborted) return;
        if (cached) {
          applyValues(cached.values);
          lastSaved.current = "";
          serverUpdatedAt.current = cached.serverUpdatedAt;
          setReady(true);
        }
        setAutosavePaused(true);
        setMessage(cause instanceof Error ? cause.message : "加载文章失败");
      }
    }
    void load();
    return () => active.abort();
  }, [publicId, session?.username]);

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

  /** 保存操作串行化；自动写入仅允许草稿，并带版本号避免覆盖其他标签页。 */
  async function persistArticle(automatic = false): Promise<AdminArticle> {
    if (!session) throw new Error("请先登录");
    if (operation.current) await operation.current;
    const captured = { ...latestValues.current };
    const previous = lastSaved.current
      ? (JSON.parse(lastSaved.current) as EditorDraftValues)
      : null;
    if (!captured.slug) {
      draftSlug.current ||= "article-" + crypto.randomUUID().slice(0, 12);
      captured.slug = draftSlug.current;
      setSlug(captured.slug);
    }
    const write = async () => {
      const article = await apiRequest<AdminArticle>(
        savedId.current ? "/api/v1/admin/articles/" + savedId.current : "/api/v1/admin/articles",
        {
          method: savedId.current ? "PUT" : "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({
            title: captured.title.trim() || "未命名文章",
            slug: captured.slug,
            summary: captured.summary || null,
            document: { type: "markdown", source: captured.source } satisfies MarkdownDocument,
            draft_only: automatic,
            expected_updated_at: serverUpdatedAt.current,
          }),
        },
      );
      savedId.current = article.public_id;
      serverUpdatedAt.current = article.updated_at;
      const coverChanged =
        !previous || previous.cover.asset_public_id !== captured.cover.asset_public_id;
      const taxonomyChanged =
        !previous ||
        JSON.stringify([previous.categories, previous.tags]) !==
          JSON.stringify([captured.categories, captured.tags]);
      if (coverChanged)
        await apiRequest("/api/v1/admin/articles/" + article.public_id + "/cover", {
          method: "PUT",
          headers: csrfHeaders(session),
          body: JSON.stringify({ asset_public_id: captured.cover.asset_public_id }),
        });
      if (taxonomyChanged)
        await apiRequest("/api/v1/admin/articles/" + article.public_id + "/taxonomy", {
          method: "PUT",
          headers: csrfHeaders(session),
          body: JSON.stringify({ categories: captured.categories, tags: captured.tags }),
        });
      const completed = coverChanged
        ? await apiRequest<AdminArticle>("/api/v1/admin/articles/" + article.public_id)
        : article;
      serverUpdatedAt.current = completed.updated_at;
      if (
        sourceFromDocument(completed.document) !== captured.source ||
        completed.slug !== captured.slug ||
        completed.title !== (captured.title.trim() || "未命名文章")
      )
        throw new Error("保存期间文章被其他窗口更新，请刷新后核对本地草稿");
      lastSaved.current = JSON.stringify(captured);
      setAutosavePaused(false);
      setAutosaveMessage(automatic ? "草稿已自动保存到服务器" : "已保存到服务器");
      return completed;
    };
    const running = write();
    operation.current = running;
    try {
      return await running;
    } finally {
      if (operation.current === running) operation.current = null;
    }
  }

  const persistRef = useRef(persistArticle);
  persistRef.current = persistArticle;
  useEffect(() => {
    if (
      !ready ||
      !dirty ||
      pending ||
      autosavePaused ||
      status !== "draft" ||
      (!title.trim() && !source.trim())
    )
      return;
    const timer = window.setTimeout(() => {
      setPending(true);
      setAutosaveMessage("正在自动保存草稿…");
      void persistRef
        .current(true)
        .catch((cause: unknown) => {
          setAutosavePaused(true);
          setAutosaveMessage(
            "自动保存暂停：" +
              (cause instanceof Error ? cause.message : "网络异常") +
              "。本地草稿仍保留，可点击保存重试。",
          );
        })
        .finally(() => setPending(false));
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [ready, dirty, fingerprint, pending, autosavePaused, status, title, source]);

  /** 手动保存新文章前把尚未完成的编辑转移到目标恢复槽位。 */
  function transferDraft(id: string) {
    if (!session || publicId) return;
    if (JSON.stringify(latestValues.current) !== lastSaved.current)
      storeEditorDraft(editorDraftKey(session.username, id), {
        version: 1,
        values: latestValues.current,
        savedId: id,
        serverUpdatedAt: serverUpdatedAt.current,
        savedAt: new Date().toISOString(),
      });
    protection.discard();
  }

  async function save() {
    setPending(true);
    setMessage("");
    try {
      const article = await persistArticle();
      setMessage("已保存");
      if (!publicId) {
        transferDraft(article.public_id);
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
      serverUpdatedAt.current = article.updated_at;
      setPublishOpen(false);
      setMessage(article.status === "published" ? "文章已发布" : "文章已撤回");
      if (!publicId) {
        transferDraft(article.public_id);
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
      protection.discard();
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
      serverUpdatedAt.current = article.updated_at;
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
      {!ready && (
        <p role="status" className="editor-draft-notice">
          正在加载文章与恢复草稿…
        </p>
      )}
      {(protection.error || autosavePaused) && (
        <p role="alert" className="editor-draft-notice">
          {protection.error || autosaveMessage || "自动保存已暂停，请核对后手动保存"}
        </p>
      )}
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
          disabled={!ready}
          onChange={(event) => setTitle(event.target.value)}
        />
        <span className="markdown-save-status" role="status">
          {message ||
            taxonomyError ||
            (pending
              ? autosaveMessage
              : dirty
                ? protection.protected
                  ? status === "published"
                    ? "已保护本地草稿 · 发布正文需手动保存"
                    : "本地草稿已保护，等待自动保存"
                  : "正在保护草稿…"
                : autosaveMessage)}
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
        <button
          type="button"
          onClick={save}
          disabled={pending || !ready}
          className="button-secondary"
        >
          <Save size={16} /> {status === "published" ? "保存修改" : "保存草稿"}
        </button>
        <button
          type="button"
          onClick={() => {
            ensureSlug();
            setMessage("");
            setPublishOpen(true);
          }}
          disabled={pending || !ready}
          className="button-primary"
        >
          {status === "draft" ? "发布" : "发布设置"}
        </button>
      </header>
      <div className="editor-writing-layout">
        <div className="editor-writing-main">
          <div className="markdown-tool-row" data-preview={previewVisible}>
            <div className="markdown-tool-content">
              <EditorToolbar onCommand={format} />
              <div className="markdown-view-controls">
                <button
                  type="button"
                  aria-pressed={previewVisible}
                  onClick={() => {
                    rememberLayout(!previewVisible, split);
                    setView("edit");
                  }}
                  title={previewVisible ? "隐藏预览" : "显示预览"}
                >
                  <PanelRight size={16} />
                  {previewVisible ? "隐藏预览" : "显示预览"}
                </button>
                <button
                  type="button"
                  data-writing-panel-trigger
                  aria-pressed={writing.panelOpen}
                  onClick={() => writing.setPanelOpen(!writing.panelOpen)}
                >
                  <Sparkles size={16} />
                  AI 对话
                </button>
              </div>
            </div>
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
            <button
              type="button"
              aria-pressed={view === "preview"}
              onClick={() => {
                setPreviewVisible(true);
                setView("preview");
              }}
            >
              预览
            </button>
          </div>
          <div
            className="editor-workspace"
            ref={workspace}
            data-preview={previewVisible}
            style={{ "--editor-split": split + "%" } as CSSProperties}
          >
            <div className="editor-pane editor-edit-pane markdown-source">
              <WritingSelectionActions assistant={writing} />
              {ready && (
                <CodeMirror
                  editable={ready}
                  value={source}
                  height="100%"
                  extensions={[...editorExtensions, ...writing.extensions]}
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
              )}
            </div>
            {previewVisible && (
              <>
                <ResizeHandle
                  label="调整源码和预览宽度"
                  value={split}
                  onChange={(width) => rememberLayout(previewVisible, width)}
                  container={workspace}
                />
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
              </>
            )}
          </div>
        </div>
        <WritingAssistant assistant={writing} />
      </div>
      <footer className="markdown-statusbar">
        <span>
          字符数 {source.length} · 行数 {lines}
        </span>
        <span role="status">{previewError || (status === "published" ? "已发布" : "草稿")}</span>
      </footer>
      {publishOpen && (
        <PublishPanel
          title={title}
          source={source}
          slug={slug}
          summary={summary}
          cover={cover}
          categories={categories}
          tags={tags}
          selectedCategories={selectedCategories}
          selectedTags={selectedTags}
          pending={pending}
          message={message || taxonomyError}
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
