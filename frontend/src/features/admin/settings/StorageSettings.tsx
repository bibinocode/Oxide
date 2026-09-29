import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../AdminSession";
import { apiRequest } from "../../../lib/api/client";

interface Provider {
  id: string;
  kind: "aliyun_oss" | "qiniu_kodo";
  name: string;
  public_base_url: string;
  endpoint: string | null;
  bucket: string | null;
  region: string | null;
  credentials_configured: boolean;
  upload_enabled: boolean;
  active: boolean;
}

export function ProviderSection() {
  const { session } = useAdminSession();
  const [providers, setProviders] = useState<Provider[]>([]);
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState<Provider["kind"]>("aliyun_oss");
  const [publicBaseUrl, setPublicBaseUrl] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [bucket, setBucket] = useState("");
  const [region, setRegion] = useState("");
  const [accessKey, setAccessKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    apiRequest<Provider[]>("/api/v1/admin/storage-providers")
      .then(setProviders)
      .catch((error) => setMessage(error.message));
  }, []);

  function editProvider(provider: Provider) {
    setEditingId(provider.id);
    setId(provider.id);
    setName(provider.name);
    setKind(provider.kind);
    setPublicBaseUrl(provider.public_base_url);
    setEndpoint(provider.endpoint ?? "");
    setBucket(provider.bucket ?? "");
    setRegion(provider.region ?? "");
    setAccessKey("");
    setSecretKey("");
  }

  async function saveProvider(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!session) return;
    try {
      const created = await apiRequest<Provider>(
        editingId
          ? `/api/v1/admin/storage-providers/${editingId}`
          : "/api/v1/admin/storage-providers",
        {
          method: editingId ? "PUT" : "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({
            id,
            name,
            kind,
            public_base_url: publicBaseUrl,
            endpoint: endpoint || null,
            bucket: bucket || null,
            region: region || null,
            access_key: accessKey || null,
            secret_key: secretKey || null,
            upload_enabled: true,
          }),
        },
      );
      setProviders((previous) =>
        editingId
          ? previous.map((item) => (item.id === editingId ? created : item))
          : [...previous, created],
      );
      setEditingId(null);
      setId("");
      setName("");
      setPublicBaseUrl("");
      setEndpoint("");
      setBucket("");
      setRegion("");
      setAccessKey("");
      setSecretKey("");
      setMessage("存储配置已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "添加失败");
    }
  }

  async function activate(provider: Provider) {
    if (!session) return;
    try {
      await apiRequest(`/api/v1/admin/storage-providers/${provider.id}/activate`, {
        method: "POST",
        headers: csrfHeaders(session),
      });
      setProviders((previous) =>
        previous.map((item) => ({ ...item, active: item.id === provider.id })),
      );
      setMessage("上传提供商已切换");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "切换失败");
    }
  }

  async function removeProvider(provider: Provider) {
    if (
      !session ||
      !window.confirm(
        `删除存储提供商「${provider.name}」？${provider.active ? "删除后需重新选择上传源。" : ""}`,
      )
    )
      return;
    try {
      await apiRequest(`/api/v1/admin/storage-providers/${encodeURIComponent(provider.id)}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      setProviders((previous) => previous.filter((item) => item.id !== provider.id));
      if (editingId === provider.id) setEditingId(null);
      setMessage("存储提供商已删除");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
    }
  }

  return (
    <section className="border-t border-line pt-10">
      <p className="eyebrow">Storage</p>
      <h2 className="mt-2 text-base font-semibold">素材存储</h2>
      <div className="mt-6 divide-y divide-line border-y border-line">
        {providers.map((provider) => (
          <div key={provider.id} className="flex flex-wrap items-center gap-4 py-4 text-sm">
            <div className="min-w-0 flex-1">
              <strong>{provider.name}</strong>
              <p className="mt-1 truncate text-muted">
                {provider.kind === "aliyun_oss" ? "阿里云 OSS" : "七牛云 Kodo"} · {provider.id}
              </p>
              <p className="mt-1 text-muted">
                {provider.bucket ?? "未填写空间"} ·{" "}
                {provider.credentials_configured ? "凭据已保存" : "凭据使用环境变量或未配置"}
              </p>
            </div>
            <button
              type="button"
              onClick={() => editProvider(provider)}
              className="button-secondary"
            >
              编辑
            </button>
            {provider.active ? (
              <span className="font-semibold text-accent">当前上传源</span>
            ) : (
              <button type="button" onClick={() => activate(provider)} className="button-secondary">
                设为当前
              </button>
            )}
            <button
              type="button"
              onClick={() => removeProvider(provider)}
              title={`删除${provider.name}`}
              aria-label={`删除${provider.name}`}
              className="text-muted hover:text-warm"
            >
              <Trash2 size={17} />
            </button>
          </div>
        ))}
      </div>
      <form onSubmit={saveProvider} className="mt-7 grid gap-3 sm:grid-cols-2">
        <label className="text-sm">
          标识
          <input
            className="field mt-2"
            placeholder="MY_OSS"
            required
            disabled={editingId !== null}
            pattern="[A-Z0-9_]{2,40}"
            value={id}
            onChange={(event) => setId(event.target.value.toUpperCase())}
          />
        </label>
        <label className="text-sm">
          名称
          <input
            className="field mt-2"
            required
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="text-sm">
          类型
          <select
            className="field mt-2"
            value={kind}
            onChange={(event) => setKind(event.target.value as Provider["kind"])}
          >
            <option value="aliyun_oss">阿里云 OSS</option>
            <option value="qiniu_kodo">七牛云 Kodo</option>
          </select>
        </label>
        <label className="text-sm">
          公开域名
          <input
            className="field mt-2"
            required
            type="url"
            placeholder="https://cdn.example.com"
            value={publicBaseUrl}
            onChange={(event) => setPublicBaseUrl(event.target.value)}
          />
        </label>
        <label className="text-sm">
          {kind === "qiniu_kodo" ? "S3 兼容端点（可选）" : "OSS 端点"}
          <input
            className="field mt-2"
            type="url"
            required={kind === "aliyun_oss"}
            placeholder="https://s3-cn-east-1.qiniucs.com"
            value={endpoint}
            onChange={(event) => setEndpoint(event.target.value)}
          />
        </label>
        <label className="text-sm">
          空间名称 Bucket
          <input
            className="field mt-2"
            required
            value={bucket}
            onChange={(event) => setBucket(event.target.value)}
          />
        </label>
        <label className="text-sm">
          {kind === "qiniu_kodo" ? "区域 Region（可选）" : "区域 Region"}
          <input
            className="field mt-2"
            required={kind === "aliyun_oss"}
            placeholder="cn-east-1"
            value={region}
            onChange={(event) => setRegion(event.target.value)}
          />
        </label>
        <label className="text-sm">
          Access Key {editingId && "（留空保留原值）"}
          <input
            className="field mt-2"
            type="password"
            autoComplete="off"
            required={!editingId}
            value={accessKey}
            onChange={(event) => setAccessKey(event.target.value)}
          />
        </label>
        <label className="text-sm">
          Secret Key {editingId && "（留空保留原值）"}
          <input
            className="field mt-2"
            type="password"
            autoComplete="off"
            required={!editingId}
            value={secretKey}
            onChange={(event) => setSecretKey(event.target.value)}
          />
        </label>
        <div className="sm:col-span-2">
          <button type="submit" className="button-primary">
            <Plus size={16} /> {editingId ? "保存提供商" : "添加提供商"}
          </button>
          {editingId && (
            <button
              type="button"
              className="button-secondary ml-2"
              onClick={() => {
                setEditingId(null);
                setId("");
                setName("");
                setPublicBaseUrl("");
                setEndpoint("");
                setBucket("");
                setRegion("");
                setAccessKey("");
                setSecretKey("");
              }}
            >
              取消编辑
            </button>
          )}
          <p role="status" className="mt-3 text-sm text-muted">
            {message}
          </p>
        </div>
      </form>
    </section>
  );
}
