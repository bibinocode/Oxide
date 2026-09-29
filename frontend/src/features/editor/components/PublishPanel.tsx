import { useEffect, useRef, useState } from "react";
import { Upload, X } from "lucide-react";
import { apiRequest } from "../../../lib/api/client";
import type { Taxonomy } from "../../../lib/api/types";
import { SummaryAssistant } from "./SummaryAssistant";

/** 封面只保存公开素材标识，正式媒体地址由服务端确定。 */
export interface CoverSelection {
  asset_public_id: string | null;
  media_url: string | null;
}
interface Asset {
  public_id: string;
  media_url: string;
  mime_type: string;
  visibility: string;
}
interface PublishPanelProps {
  title: string;
  source: string;
  slug: string;
  summary: string;
  cover: CoverSelection;
  categories: Taxonomy[];
  tags: Taxonomy[];
  selectedCategories: string[];
  selectedTags: string[];
  pending: boolean;
  message: string;
  published: boolean;
  onSlug: (value: string) => void;
  onSummary: (value: string) => void;
  onCover: (value: CoverSelection) => void;
  onCategories: (value: string[]) => void;
  onTags: (value: string[]) => void;
  onUpload: (file: File) => Promise<CoverSelection>;
  onPublish: () => void;
  onClose: () => void;
}

/** 发布时再补充摘要、封面和分类，日常写作界面只保留正文。 */
export function PublishPanel(props: PublishPanelProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    dialog.current?.showModal();
    apiRequest<Asset[]>("/api/v1/admin/assets")
      .then((items) =>
        setAssets(
          items.filter(
            (item) => item.visibility === "public" && item.mime_type.startsWith("image/"),
          ),
        ),
      )
      .catch((cause) => setError(cause.message));
  }, []);
  async function upload(file: File) {
    setUploading(true);
    setError("");
    try {
      props.onCover(await props.onUpload(file));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "封面上传失败");
    } finally {
      setUploading(false);
    }
  }
  return (
    <dialog
      ref={dialog}
      className="publish-dialog"
      onCancel={props.onClose}
      aria-labelledby="publish-heading"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          props.onPublish();
        }}
      >
        <header className="publish-heading">
          <div>
            <p className="eyebrow">Publish</p>
            <h2 id="publish-heading" className="mt-2 text-lg font-semibold">
              准备发布
            </h2>
          </div>
          <button type="button" aria-label="返回写作" onClick={props.onClose}>
            <X size={20} />
          </button>
        </header>
        <div className="publish-fields">
          <label className="block text-sm">
            文章链接
            <input
              autoFocus
              required
              pattern="[a-z0-9]+(?:-[a-z0-9]+)*"
              minLength={3}
              maxLength={100}
              className="field mt-2"
              value={props.slug}
              onChange={(e) => props.onSlug(e.target.value.toLowerCase())}
            />
          </label>
          <label className="block text-sm">
            摘要
            <textarea
              className="field mt-2 min-h-24"
              maxLength={500}
              placeholder="用几句话介绍这篇文章（选填）"
              value={props.summary}
              onChange={(e) => props.onSummary(e.target.value)}
            />
          </label>
          <SummaryAssistant title={props.title} source={props.source} onApply={props.onSummary} />
          <fieldset className="space-y-3">
            <legend className="mb-3 text-sm">文章主图</legend>
            {props.cover.media_url && (
              <div className="publish-cover">
                <img src={props.cover.media_url} alt="当前主图" />
                <button
                  type="button"
                  onClick={() => props.onCover({ asset_public_id: null, media_url: null })}
                >
                  移除主图
                </button>
              </div>
            )}
            <label className="button-secondary cursor-pointer">
              <Upload size={15} />
              {uploading ? "正在上传…" : "上传主图"}
              <input
                type="file"
                className="sr-only"
                accept="image/png,image/jpeg,image/webp,image/gif"
                disabled={uploading}
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) void upload(file);
                  event.target.value = "";
                }}
              />
            </label>
            {assets.length > 0 && (
              <details>
                <summary className="cursor-pointer text-xs text-muted">从素材库选择</summary>
                <div className="mt-3 grid grid-cols-3 gap-2">
                  {assets.map((asset) => (
                    <button
                      key={asset.public_id}
                      type="button"
                      aria-label={`选择主图 ${asset.public_id}`}
                      aria-pressed={props.cover.asset_public_id === asset.public_id}
                      className="cover-choice"
                      onClick={() =>
                        props.onCover({
                          asset_public_id: asset.public_id,
                          media_url: asset.media_url,
                        })
                      }
                    >
                      <img src={asset.media_url} alt="素材缩略图" loading="lazy" />
                    </button>
                  ))}
                </div>
              </details>
            )}
          </fieldset>
          <div className="grid grid-cols-2 gap-6">
            <TaxonomyChoices
              label="分类"
              items={props.categories}
              selected={props.selectedCategories}
              onChange={props.onCategories}
            />
            <TaxonomyChoices
              label="标签"
              items={props.tags}
              selected={props.selectedTags}
              onChange={props.onTags}
            />
          </div>
          {(error || props.message) && (
            <p role="status" className="text-sm text-warm">
              {error || props.message}
            </p>
          )}
        </div>
        <footer className="publish-actions">
          <button type="button" className="button-secondary" onClick={props.onClose}>
            继续写作
          </button>
          <button type="submit" disabled={props.pending || uploading} className="button-primary">
            {props.pending ? "正在保存…" : props.published ? "更新发布" : "确认发布"}
          </button>
        </footer>
      </form>
    </dialog>
  );
}

/** 受控选择列表，允许文章暂时没有分类或标签。 */
function TaxonomyChoices({
  label,
  items,
  selected,
  onChange,
}: {
  label: string;
  items: Taxonomy[];
  selected: string[];
  onChange: (value: string[]) => void;
}) {
  return (
    <fieldset>
      <legend className="mb-3 text-sm">{label}</legend>
      <div className="space-y-2">
        {items.map((item) => (
          <label key={item.slug} className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={selected.includes(item.slug)}
              onChange={(e) =>
                onChange(
                  e.target.checked
                    ? [...selected, item.slug]
                    : selected.filter((value) => value !== item.slug),
                )
              }
            />
            {item.name}
          </label>
        ))}
        {items.length === 0 && (
          <p className="text-xs text-muted">暂无{label}，可稍后在站点设置中添加</p>
        )}
      </div>
    </fieldset>
  );
}
