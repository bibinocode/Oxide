import { ImagePlus, X } from "lucide-react";
import { useEffect, useState } from "react";
import { HomeIntroduction } from "../../home/HomeIntroduction";
import { csrfHeaders } from "../AdminSession";
import { apiRequest } from "../../../lib/api/client";
import type { Session, SiteSettings } from "../../../lib/api/types";
import { emptyXiaohongshuCard } from "../../../lib/api/types";
import { HomeIntroRichEditor } from "./HomeIntroRichEditor";
import { parseProfile } from "../../contact/profile";

interface PortraitAsset {
  public_id: string;
  media_url: string;
  mime_type: string;
  visibility: "public" | "private";
}

/** 图片先上传并设为公开，再将稳定地址写入首页设置。 */
export function HomeIntroductionEditor({
  site,
  session,
  onChange,
}: {
  site: SiteSettings;
  session: Session | null;
  onChange: (value: SiteSettings) => void;
}) {
  const [assets, setAssets] = useState<PortraitAsset[]>([]);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState("");
  const intro = site.presentation.home_intro;
  const xiaohongshu = intro.xiaohongshu ?? emptyXiaohongshuCard();

  useEffect(() => {
    apiRequest<PortraitAsset[]>("/api/v1/admin/assets")
      .then((items) =>
        setAssets(
          items.filter(
            (item) => item.visibility === "public" && item.mime_type.startsWith("image/"),
          ),
        ),
      )
      .catch((cause) => setError(cause instanceof Error ? cause.message : "素材读取失败"));
  }, []);

  function update(patch: Partial<typeof intro>) {
    onChange({
      ...site,
      presentation: {
        ...site.presentation,
        home_intro: { ...intro, ...patch },
      },
    });
  }

  function updateXiaohongshu(patch: Partial<typeof xiaohongshu>) {
    update({ xiaohongshu: { ...xiaohongshu, ...patch } });
  }

  async function upload(file: File) {
    if (!session) return;
    setUploading(true);
    setError("");
    try {
      const body = new FormData();
      body.append("file", file);
      const created = await apiRequest<PortraitAsset>("/api/v1/admin/assets", {
        method: "POST",
        headers: csrfHeaders(session),
        body,
      });
      const published = await apiRequest<PortraitAsset>(
        `/api/v1/admin/assets/${created.public_id}`,
        {
          method: "PATCH",
          headers: csrfHeaders(session),
          body: JSON.stringify({ public: true }),
        },
      );
      setAssets((previous) => [published, ...previous]);
      update({ portrait_url: published.media_url, portrait_alt: intro.portrait_alt || "个人肖像" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "照片上传失败");
    } finally {
      setUploading(false);
    }
  }

  return (
    <div className="home-intro-editor">
      <div className="home-intro-editor-fields">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={intro.enabled}
            onChange={(event) => update({ enabled: event.target.checked })}
          />
          在首页展示个人介绍
        </label>
        <div className="text-sm">
          <p className="mb-2">介绍正文</p>
          <HomeIntroRichEditor value={intro.body} onChange={(body) => update({ body })} />
          <p className="mt-2 text-xs leading-6 text-muted">
            像首页一样分段介绍自己：Enter 新建段落，Shift + Enter
            换行；选中文字可强调或添加链接。社交主页与邮箱链接支持悬停资料卡。
          </p>
          <p className="mt-2 text-right text-xs text-muted">{intro.body.length} / 2000</p>
        </div>
        <div className="home-intro-profile-fields border-t border-line pt-5">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium">小红书资料卡</p>
            {xiaohongshu.url && (
              <button
                type="button"
                className="text-muted hover:text-ink"
                title="清空小红书资料卡"
                aria-label="清空小红书资料卡"
                onClick={() => update({ xiaohongshu: emptyXiaohongshuCard() })}
              >
                <X size={16} />
              </button>
            )}
          </div>
          <label className="block text-sm">
            个人主页链接
            <input
              className="field mt-2"
              type="url"
              maxLength={2048}
              value={xiaohongshu.url}
              onChange={(event) =>
                updateXiaohongshu({
                  url: parseProfile(event.target.value)?.url ?? event.target.value,
                })
              }
              placeholder="https://www.xiaohongshu.com/user/profile/..."
            />
          </label>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="block text-sm">
              显示名称
              <input
                className="field mt-2"
                maxLength={80}
                value={xiaohongshu.name}
                onChange={(event) => updateXiaohongshu({ name: event.target.value })}
              />
            </label>
            <label className="block text-sm">
              小红书号
              <input
                className="field mt-2"
                maxLength={80}
                value={xiaohongshu.handle}
                onChange={(event) => updateXiaohongshu({ handle: event.target.value })}
              />
            </label>
          </div>
          <label className="block text-sm">
            个人简介
            <textarea
              className="field mt-2 min-h-24"
              maxLength={500}
              value={xiaohongshu.bio}
              onChange={(event) => updateXiaohongshu({ bio: event.target.value })}
            />
          </label>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="block text-sm">
              粉丝数
              <input
                className="field mt-2"
                maxLength={32}
                value={xiaohongshu.followers}
                onChange={(event) => updateXiaohongshu({ followers: event.target.value })}
                placeholder="10+"
              />
            </label>
            <label className="block text-sm">
              获赞与收藏
              <input
                className="field mt-2"
                maxLength={32}
                value={xiaohongshu.likes}
                onChange={(event) => updateXiaohongshu({ likes: event.target.value })}
                placeholder="1千+"
              />
            </label>
          </div>
        </div>
        <div className="border-t border-line pt-5">
          <div className="mb-3 flex items-center justify-between">
            <span className="text-sm font-medium">肖像照片</span>
            {intro.portrait_url && (
              <button
                type="button"
                className="text-muted hover:text-ink"
                title="移除照片"
                aria-label="移除照片"
                onClick={() => update({ portrait_url: "" })}
              >
                <X size={16} />
              </button>
            )}
          </div>
          <label className="button-secondary inline-flex cursor-pointer items-center gap-2">
            <ImagePlus size={16} /> {uploading ? "上传中…" : "上传照片"}
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif"
              className="hidden"
              disabled={uploading}
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) void upload(file);
                event.target.value = "";
              }}
            />
          </label>
          {assets.length > 0 && (
            <label className="mt-4 block text-sm">
              或选择已公开素材
              <select
                className="field mt-2"
                value={intro.portrait_url}
                onChange={(event) => update({ portrait_url: event.target.value })}
              >
                <option value="">不使用照片</option>
                {assets.map((asset) => (
                  <option key={asset.public_id} value={asset.media_url}>
                    {asset.public_id}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="mt-4 block text-sm">
            图片说明
            <input
              className="field mt-2"
              maxLength={120}
              value={intro.portrait_alt}
              onChange={(event) => update({ portrait_alt: event.target.value })}
              placeholder="个人肖像"
            />
          </label>
        </div>
        <p role="alert" className="text-sm text-warm">
          {error}
        </p>
      </div>
      <div className="home-intro-editor-preview">
        <p className="eyebrow mb-8">首页预览</p>
        <HomeIntroduction name={site.site_name} description={site.description} value={intro} />
      </div>
    </div>
  );
}
