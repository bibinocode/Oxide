import { createFileRoute } from "@tanstack/react-router";
import { Copy, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";
import { groupByDate } from "../lib/utils/dateGroups";

interface Asset {
  public_id: string;
  media_url: string;
  provider_id: string;
  object_key: string;
  mime_type: string;
  size_bytes: number;
  width: number | null;
  height: number | null;
  visibility: "public" | "private";
  created_at: string;
}

export const Route = createFileRoute("/admin/assets")({ component: AssetsPage });

function AssetsPage() {
  const { session } = useAdminSession();
  const [assets, setAssets] = useState<Asset[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const months = groupByDate(assets, (asset) => asset.created_at, "month");

  useEffect(() => {
    const controller = new AbortController();
    apiRequest<Asset[]>("/api/v1/admin/assets", { signal: controller.signal })
      .then((items) => {
        if (controller.signal.aborted) return;
        setAssets(items);
        setHasMore(items.length === 100);
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted)
          setMessage(error instanceof Error ? error.message : "素材加载失败");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, []);

  /** 由末条真实记录继续查询，加载早期素材时保留已有年月分组。 */
  async function loadMore() {
    const last = assets.at(-1);
    if (loading) return;
    setLoading(true);
    setMessage("");
    try {
      const items = await apiRequest<Asset[]>(
        last
          ? `/api/v1/admin/assets?before=${encodeURIComponent(last.public_id)}`
          : "/api/v1/admin/assets",
      );
      setAssets((current) => {
        const existing = new Set(current.map((item) => item.public_id));
        return [...current, ...items.filter((item) => !existing.has(item.public_id))];
      });
      setHasMore(items.length === 100);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "历史素材加载失败");
    } finally {
      setLoading(false);
    }
  }

  async function upload(file: File) {
    if (!session) return;
    setBusy(true);
    setMessage("");
    const body = new FormData();
    body.append("file", file);
    try {
      const created = await apiRequest<Asset>("/api/v1/admin/assets", {
        method: "POST",
        headers: csrfHeaders(session),
        body,
      });
      setAssets((previous) => [created, ...previous]);
      setMessage("素材已上传");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "上传失败");
    } finally {
      setBusy(false);
    }
  }

  async function toggle(asset: Asset) {
    if (!session) return;
    try {
      const updated = await apiRequest<Asset>(`/api/v1/admin/assets/${asset.public_id}`, {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ public: asset.visibility !== "public" }),
      });
      setAssets((previous) =>
        previous.map((item) => (item.public_id === updated.public_id ? updated : item)),
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "更新失败");
    }
  }

  async function remove(asset: Asset) {
    if (!session || !window.confirm("删除该素材？")) return;
    try {
      await apiRequest(`/api/v1/admin/assets/${asset.public_id}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      setAssets((previous) => previous.filter((item) => item.public_id !== asset.public_id));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
    }
  }

  return (
    <section className="pt-9">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <p className="eyebrow">Media</p>
          <h2 className="mt-2 text-base font-semibold">素材库</h2>
          <p className="mt-2 text-sm text-muted">按上传年月归档 · 已加载 {assets.length} 项</p>
        </div>
        <label className="button-primary">
          <Upload size={16} /> {busy ? "上传中…" : "上传图片"}
          <input
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp"
            className="hidden"
            disabled={busy || loading}
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) void upload(file);
              event.target.value = "";
            }}
          />
        </label>
      </div>
      <p role="status" className="mt-4 text-sm text-muted">
        {message}
      </p>
      <div className="mt-7 space-y-9">
        {months.map((month) => (
          <section key={month.key} aria-labelledby={`assets-${month.key}`}>
            <h3
              id={`assets-${month.key}`}
              className="mb-4 flex items-center gap-3 border-b border-line pb-3 text-sm font-medium"
            >
              {month.label}
              <span className="font-mono text-xs font-normal text-muted">
                {month.items.length} 项
              </span>
            </h3>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              {month.items.map((asset) => (
                <article
                  key={asset.public_id}
                  className="overflow-hidden rounded-panel border border-line bg-surface"
                >
                  <div className="aspect-[4/3] bg-paper">
                    <img
                      src={asset.media_url}
                      alt="已上传素材"
                      className="h-full w-full object-contain"
                      loading="lazy"
                    />
                  </div>
                  <div className="space-y-3 p-3">
                    <p className="truncate text-xs text-muted">
                      {asset.mime_type} · {Math.round(asset.size_bytes / 1024)} KB
                    </p>
                    <details className="text-xs text-muted">
                      <summary className="cursor-pointer">存储位置</summary>
                      <dl className="mt-2 space-y-1 break-all">
                        <dt>提供商</dt>
                        <dd>{asset.provider_id}</dd>
                        <dt>对象路径</dt>
                        <dd className="font-mono">{asset.object_key}</dd>
                      </dl>
                    </details>
                    <label className="flex items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        checked={asset.visibility === "public"}
                        onChange={() => toggle(asset)}
                      />{" "}
                      公开访问
                    </label>
                    <div className="flex items-center gap-3">
                      <button
                        type="button"
                        title="复制链接"
                        aria-label="复制链接"
                        onClick={() =>
                          navigator.clipboard
                            .writeText(`${window.location.origin}${asset.media_url}`)
                            .then(() => setMessage("链接已复制"))
                        }
                        className="text-muted hover:text-accent"
                      >
                        <Copy size={17} />
                      </button>
                      <button
                        type="button"
                        title="删除素材"
                        aria-label="删除素材"
                        disabled={loading}
                        onClick={() => remove(asset)}
                        className="text-muted hover:text-warm"
                      >
                        <Trash2 size={17} />
                      </button>
                    </div>
                  </div>
                </article>
              ))}
            </div>
          </section>
        ))}
      </div>
      {hasMore && (
        <button
          type="button"
          className="button-secondary mt-8"
          disabled={loading || busy}
          onClick={() => void loadMore()}
        >
          {loading ? "正在加载…" : "加载更早的素材"}
        </button>
      )}
      {loading && assets.length === 0 && (
        <p role="status" className="mt-8 text-sm text-muted">
          正在加载素材…
        </p>
      )}
      {!loading && !message && assets.length === 0 && (
        <p className="mt-8 border-t border-line py-12 text-sm text-muted">暂无素材。</p>
      )}
    </section>
  );
}
