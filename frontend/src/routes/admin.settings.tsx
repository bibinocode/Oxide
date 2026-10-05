import { Link, createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";
import type { SiteSettings, Taxonomy } from "../lib/api/types";
import { emptyPresentation } from "../lib/api/types";
import { SiteModulesEditor } from "../features/admin/settings/SiteModulesEditor";
import { TaxonomyEditor } from "../features/admin/settings/TaxonomyEditor";
import { ProviderSection } from "../features/admin/settings/StorageSettings";
import { HomeIntroductionEditor } from "../features/admin/home/HomeIntroductionEditor";
import { AboutEditor } from "../features/admin/settings/AboutEditor";

export const Route = createFileRoute("/admin/settings")({ component: SettingsPage });

function SettingsPage() {
  const { session } = useAdminSession();
  const [section, setSection] = useState<
    "site" | "intro" | "about" | "modules" | "taxonomy" | "storage" | "agent"
  >("site");
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
      apiRequest<Taxonomy[]>("/api/v1/admin/categories"),
      apiRequest<Taxonomy[]>("/api/v1/admin/tags"),
    ])
      .then(([settings, categoryItems, tagItems]) => {
        setSite({
          ...settings,
          presentation: { ...emptyPresentation(), ...settings.presentation },
        });
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
    if (!session) return false;
    try {
      const created = await apiRequest<Taxonomy>(`/api/v1/admin/${kind}`, {
        method: "POST",
        headers: csrfHeaders(session),
        body: JSON.stringify(item),
      });
      if (kind === "categories") setCategories((previous) => [...previous, created]);
      else setTags((previous) => [...previous, created]);
      setMessage("已创建");
      return true;
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "创建失败");
      return false;
    }
  }

  async function remove(kind: "categories" | "tags", slug: string) {
    if (!session || !window.confirm(`删除「${slug}」并移除其文章关联？文章正文会保留。`))
      return false;
    try {
      await apiRequest(`/api/v1/admin/${kind}/${slug}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      if (kind === "categories")
        setCategories((previous) => previous.filter((item) => item.slug !== slug));
      else setTags((previous) => previous.filter((item) => item.slug !== slug));
      setMessage("条目及关联已删除，文章正文已保留");
      return true;
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
      return false;
    }
  }

  async function visibility(kind: "categories" | "tags", item: Taxonomy) {
    if (!session) return false;
    try {
      const value = await apiRequest<Taxonomy>(`/api/v1/admin/${kind}/${item.slug}`, {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ visible: !item.visible }),
      });
      const update = (items: Taxonomy[]) =>
        items.map((entry) => (entry.slug === item.slug ? value : entry));
      if (kind === "categories") setCategories(update);
      else setTags(update);
      setMessage(value.visible ? "已恢复公开展示" : "已隐藏，文章关联仍保留");
      return true;
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : "状态修改失败");
      return false;
    }
  }

  return (
    <section className="pt-9">
      <nav className="mb-9 flex flex-wrap gap-2 border-b border-line pb-4" aria-label="设置分类">
        {(
          [
            ["site", "站点信息"],
            ["intro", "首页介绍"],
            ["about", "关于我"],
            ["modules", "页面模块"],
            ["taxonomy", "分类与标签"],
            ["storage", "素材存储"],
            ["agent", "AI 与 Agent"],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            type="button"
            onClick={() => setSection(key)}
            aria-current={section === key ? "page" : undefined}
            className={section === key ? "button-primary" : "button-secondary"}
          >
            {label}
          </button>
        ))}
      </nav>
      {section === "site" && (
        <div className="max-w-3xl">
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
            <button type="submit" className="button-primary">
              保存设置
            </button>
          </form>
          <p role="status" className="mt-4 text-sm text-muted">
            {message}
          </p>
        </div>
      )}
      {section === "modules" && (
        <div className="max-w-3xl">
          <p className="eyebrow">Presentation</p>
          <h2 className="mt-2 text-base font-semibold">页面模块</h2>
          <form onSubmit={saveSite} className="mt-7 space-y-5">
            <SiteModulesEditor
              value={site.presentation}
              onChange={(presentation) => setSite({ ...site, presentation })}
            />
            <button type="submit" className="button-primary">
              保存页面模块
            </button>
          </form>
          <p role="status" className="mt-4 text-sm text-muted">
            {message}
          </p>
        </div>
      )}
      {section === "intro" && (
        <div>
          <p className="eyebrow">Introduction</p>
          <h2 className="mt-2 text-base font-semibold">首页个人介绍</h2>
          <form onSubmit={saveSite} className="mt-7">
            <HomeIntroductionEditor site={site} session={session} onChange={setSite} />
            <button type="submit" className="button-primary mt-6">
              保存首页介绍
            </button>
          </form>
          <p role="status" className="mt-4 text-sm text-muted">
            {message}
          </p>
        </div>
      )}
      {section === "about" && (
        <div>
          <p className="eyebrow">About</p>
          <h2 className="mt-2 text-base font-semibold">关于我</h2>
          <form onSubmit={saveSite} className="mt-7 space-y-5">
            <AboutEditor
              value={site.presentation.about_body ?? ""}
              onChange={(about_body) =>
                setSite({ ...site, presentation: { ...site.presentation, about_body } })
              }
            />
            <button type="submit" className="button-primary">
              保存关于我
            </button>
          </form>
          <p role="status" className="mt-4 text-sm text-muted">
            {message}
          </p>
        </div>
      )}
      {section === "taxonomy" && (
        <div className="grid max-w-5xl gap-10 lg:grid-cols-2">
          <TaxonomyEditor
            title="分类"
            kind="categories"
            items={categories}
            onCreate={create}
            onRemove={remove}
            onVisibility={visibility}
          />
          <TaxonomyEditor
            title="标签"
            kind="tags"
            items={tags}
            onCreate={create}
            onRemove={remove}
            onVisibility={visibility}
          />
        </div>
      )}
      {section === "storage" && (
        <div className="max-w-5xl">
          <ProviderSection />
        </div>
      )}
      {section === "agent" && (
        <div className="max-w-5xl">
          <h2 className="text-lg font-semibold">Agent 配置已迁移到独立工作区</h2>
          <p className="my-4 text-sm text-muted">
            在同一入口管理 Skills、模型与任务，后续也将在这里配置 Agent 工具。
          </p>
          <Link to="/admin/agent" search={{ tab: "models" }} className="button-primary">
            打开 Agent 配置
          </Link>
        </div>
      )}
    </section>
  );
}
