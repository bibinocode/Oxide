import { createFileRoute } from "@tanstack/react-router";
import { Copy, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";

interface Asset {
  public_id: string;
  media_url: string;
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

  useEffect(() => {
    apiRequest<Asset[]>("/api/v1/admin/assets")
      .then(setAssets)
      .catch((error) => setMessage(error.message));
  }, []);

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
        </div>
        <label className="button-primary">
          <Upload size={16} /> {busy ? "上传中…" : "上传图片"}
          <input
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp"
            className="hidden"
            disabled={busy}
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
      <div className="mt-7 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {assets.map((asset) => (
          <article
            key={asset.public_id}
            className="overflow-hidden rounded-[6px] border border-line bg-white"
          >
            <div className="aspect-[4/3] bg-paper">
              <img
                src={asset.media_url}
                alt="已上传素材"
                className="h-full w-full object-contain"
              />
            </div>
            <div className="space-y-3 p-3">
              <p className="truncate text-xs text-muted">
                {asset.mime_type} · {Math.round(asset.size_bytes / 1024)} KB
              </p>
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
      {assets.length === 0 && (
        <p className="mt-8 border-t border-line py-12 text-sm text-muted">暂无素材。</p>
      )}
    </section>
  );
}
