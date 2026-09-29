import { createFileRoute } from "@tanstack/react-router";
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";
import type { SiteSettings, Taxonomy } from "../lib/api/types";
import { emptyPresentation } from "../lib/api/types";
import { SiteModulesEditor } from "../features/admin/SiteModulesEditor";
import { AgentSettings } from "../features/admin/AgentSettings";

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

export const Route = createFileRoute("/admin/settings")({ component: SettingsPage });

function SettingsPage() {
  const { session } = useAdminSession();
  const [site, setSite] = useState<SiteSettings>({
    site_name: "Oxide",
    description: "",
    base_url: "http://127.0.0.1:3000",
    presentation: emptyPresentation(),
  });
  const [categories, setCategories] = useState<Taxonomy[]>([]);
  const [tags, setTags] = useState<Taxonomy[]>([]);
  const [message, setMessage] = useState("");

  useEffect(() => {
    Promise.all([
      apiRequest<SiteSettings>("/api/v1/site"),
      apiRequest<Taxonomy[]>("/api/v1/categories"),
      apiRequest<Taxonomy[]>("/api/v1/tags"),
    ])
      .then(([settings, categoryItems, tagItems]) => {
        setSite({ ...settings, presentation: settings.presentation ?? emptyPresentation() });
        setCategories(categoryItems);
        setTags(tagItems);
      })
      .catch((error) => setMessage(error.message));
  }, []);

  async function saveSite(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!session) return;
    try {
      setSite(
        await apiRequest<SiteSettings>("/api/v1/admin/site", {
          method: "PUT",
          headers: csrfHeaders(session),
          body: JSON.stringify(site),
        }),
      );
      setMessage("站点设置已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "保存失败");
    }
  }

  async function create(kind: "categories" | "tags", item: Taxonomy) {
    if (!session) return;
    try {
      const created = await apiRequest<Taxonomy>(`/api/v1/admin/${kind}`, {
        method: "POST",
        headers: csrfHeaders(session),
        body: JSON.stringify(item),
      });
      if (kind === "categories") setCategories((previous) => [...previous, created]);
      else setTags((previous) => [...previous, created]);
      setMessage("已创建");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "创建失败");
    }
  }

  async function remove(kind: "categories" | "tags", slug: string) {
    if (!session || !window.confirm(`删除 ${slug}？`)) return;
    try {
      await apiRequest(`/api/v1/admin/${kind}/${slug}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      if (kind === "categories")
        setCategories((previous) => previous.filter((item) => item.slug !== slug));
      else setTags((previous) => previous.filter((item) => item.slug !== slug));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
    }
  }

  return (
    <section className="grid gap-12 pt-9 lg:grid-cols-2">
      <div>
        <p className="eyebrow">Site</p>
        <h2 className="mt-2 text-base font-semibold">站点信息</h2>
        <form onSubmit={saveSite} className="mt-7 space-y-5">
          <label className="block text-sm">
            站点名称
            <input
              className="field mt-2"
              required
              maxLength={80}
              value={site.site_name}
              onChange={(event) => setSite({ ...site, site_name: event.target.value })}
            />
          </label>
          <label className="block text-sm">
            简介
            <textarea
              className="field mt-2 min-h-28"
              maxLength={500}
              value={site.description ?? ""}
              onChange={(event) => setSite({ ...site, description: event.target.value })}
            />
          </label>
          <label className="block text-sm">
            公开 URL
            <input
              className="field mt-2"
              type="url"
              required
              value={site.base_url}
              onChange={(event) => setSite({ ...site, base_url: event.target.value })}
            />
          </label>
          <SiteModulesEditor
            value={site.presentation}
            onChange={(presentation) => setSite({ ...site, presentation })}
          />
          <button type="submit" className="button-primary">
            保存设置
          </button>
        </form>
        <p role="status" className="mt-4 text-sm text-muted">
          {message}
        </p>
      </div>
      <div className="space-y-10">
        <TaxonomyEditor
          title="分类"
          kind="categories"
          items={categories}
          onCreate={create}
          onRemove={remove}
        />
        <TaxonomyEditor title="标签" kind="tags" items={tags} onCreate={create} onRemove={remove} />
      </div>
      <div className="lg:col-span-2">
        <ProviderSection />
      </div>
      <div className="lg:col-span-2">
        <AgentSettings />
      </div>
    </section>
  );
}

function TaxonomyEditor({
  title,
  kind,
  items,
  onCreate,
  onRemove,
}: {
  title: string;
  kind: "categories" | "tags";
  items: Taxonomy[];
  onCreate: (kind: "categories" | "tags", item: Taxonomy) => Promise<void>;
  onRemove: (kind: "categories" | "tags", slug: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onCreate(kind, { name, slug });
    setName("");
    setSlug("");
  }
  return (
    <div>
      <h2 className="text-base font-semibold">{title}</h2>
      <div className="mt-5 border-t border-line">
        {items.map((item) => (
          <div
            key={item.slug}
            className="flex items-center justify-between border-b border-line py-3 text-sm"
          >
            <span>
              {item.name} <span className="ml-2 text-muted">/{item.slug}</span>
            </span>
            <button
              type="button"
              onClick={() => onRemove(kind, item.slug)}
              title={`删除${item.name}`}
              aria-label={`删除${item.name}`}
              className="text-muted hover:text-warm"
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
      <form onSubmit={submit} className="mt-4 grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
        <input
          className="field"
          placeholder="名称"
          aria-label={`${title}名称`}
          required
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <input
          className="field"
          placeholder="slug"
          aria-label={`${title} slug`}
          required
          value={slug}
          onChange={(event) => setSlug(event.target.value.toLowerCase())}
        />
        <button
          type="submit"
          className="button-secondary"
          title={`添加${title}`}
          aria-label={`添加${title}`}
        >
          <Plus size={18} />
        </button>
      </form>
    </div>
  );
}

function ProviderSection() {
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
          S3 兼容端点
          <input
            className="field mt-2"
            type="url"
            required
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
          区域 Region
          <input
            className="field mt-2"
            required
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
