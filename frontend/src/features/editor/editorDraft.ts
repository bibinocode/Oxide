import type { CoverSelection } from "./components/PublishPanel";

/** 本地恢复只保存编辑内容，不保存模型密钥、AI 图片、会话 Cookie 或发布状态。 */
export interface EditorDraftValues {
  title: string;
  slug: string;
  summary: string;
  source: string;
  cover: CoverSelection;
  categories: string[];
  tags: string[];
}
export interface StoredEditorDraft {
  version: 1;
  savedId?: string;
  serverUpdatedAt: string | null;
  savedAt: string;
  values: EditorDraftValues;
}

/** 草稿以管理员与文章隔离；未建库的文章使用独立 new 槽位。 */
export function editorDraftKey(username: string, publicId?: string) {
  return "oxide.editor.draft.v1:" + encodeURIComponent(username) + ":" + (publicId ?? "new");
}

/** 外部或旧版缓存不可信，验证结构后才能恢复，错误由编辑器展示。 */
export function readEditorDraft(key: string): StoredEditorDraft | null {
  const text = localStorage.getItem(key);
  if (!text) return null;
  if (text.length > 2_000_000) throw new Error("本地草稿过大，请先导出或核对浏览器存储");
  const draft = JSON.parse(text) as StoredEditorDraft;
  const value = draft.values;
  if (
    draft.version !== 1 ||
    !value ||
    ![value.title, value.slug, value.summary, value.source].every(
      (item) => typeof item === "string",
    ) ||
    !value.cover ||
    ![value.categories, value.tags].every(
      (items) => Array.isArray(items) && items.every((item) => typeof item === "string"),
    ) ||
    (draft.savedId !== undefined && !/^[a-f0-9-]{36}$/i.test(draft.savedId))
  )
    throw new Error("本地草稿格式异常，未覆盖已有缓存");
  return draft;
}

/** 写入失败不能冒充已保存；调用方需保留离开提醒和手动保存入口。 */
export function storeEditorDraft(key: string, draft: StoredEditorDraft) {
  const text = JSON.stringify(draft);
  if (text.length > 2_000_000) throw new Error("本地草稿过大，请手动保存到服务器");
  localStorage.setItem(key, text);
}
