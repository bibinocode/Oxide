import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { csrfHeaders, useAdminSession } from "../AdminSession";
import { apiRequest } from "../../../lib/api/client";

interface AgentProvider {
  id: string;
  adapter: "openai_compatible" | "jimeng";
  name: string;
  base_url: string;
  model_id: string;
  capability: "text" | "image";
  api_key_configured: boolean;
  enabled: boolean;
}

interface Binding {
  task: string;
  provider_id: string;
}

const tasks = [
  { id: "writing", name: "写作", capability: "text" },
  { id: "summary", name: "摘要", capability: "text" },
  { id: "image", name: "生图", capability: "image" },
  { id: "chat", name: "对话", capability: "text" },
] as const;

const empty: AgentProvider = {
  id: "",
  adapter: "openai_compatible",
  name: "",
  base_url: "https://api.openai.com/v1",
  model_id: "",
  capability: "text",
  api_key_configured: false,
  enabled: true,
};

/** Agent 模型注册与任务路由；密钥只进入写请求，不保存在浏览器持久化存储。 */
export function AgentSettings() {
  const { session } = useAdminSession();
  const [providers, setProviders] = useState<AgentProvider[]>([]);
  const [bindings, setBindings] = useState<Binding[]>([]);
  const [draft, setDraft] = useState<AgentProvider>(empty);
  const [apiKey, setApiKey] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    Promise.all([
      apiRequest<AgentProvider[]>("/api/v1/admin/agent/providers"),
      apiRequest<Binding[]>("/api/v1/admin/agent/bindings"),
    ])
      .then(([models, routes]) => {
        setProviders(models);
        setBindings(routes);
      })
      .catch((error) => setMessage(error.message));
  }, []);

  function edit(provider: AgentProvider) {
    setEditingId(provider.id);
    setDraft(provider);
    setApiKey("");
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!session) return;
    try {
      const result = await apiRequest<AgentProvider>(
        editingId ? `/api/v1/admin/agent/providers/${editingId}` : "/api/v1/admin/agent/providers",
        {
          method: editingId ? "PUT" : "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({ ...draft, api_key: apiKey || null }),
        },
      );
      setProviders((items) =>
        editingId
          ? items.map((item) => (item.id === editingId ? result : item))
          : [...items, result],
      );
      setDraft(empty);
      setEditingId(null);
      setApiKey("");
      setMessage("模型配置已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "保存失败");
    }
  }

  async function bind(task: string, providerId: string) {
    if (!session || !providerId) return;
    try {
      const result = await apiRequest<Binding>(`/api/v1/admin/agent/bindings/${task}`, {
        method: "PUT",
        headers: csrfHeaders(session),
        body: JSON.stringify({ provider_id: providerId }),
      });
      setBindings((items) => [...items.filter((item) => item.task !== task), result]);
      setMessage("任务模型已切换");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "绑定失败");
    }
  }

  async function remove(id: string) {
    if (!session || !window.confirm(`删除模型 ${id}？`)) return;
    try {
      await apiRequest(`/api/v1/admin/agent/providers/${id}`, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      setProviders((items) => items.filter((item) => item.id !== id));
      setBindings((items) => items.filter((item) => item.provider_id !== id));
      setMessage("模型已删除");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "删除失败");
    }
  }

  return (
    <section className="border-t border-line pt-10">
      <p className="eyebrow">Agent</p>
      <h2 className="mt-2 text-base font-semibold">模型与任务</h2>
      <div className="mt-6 divide-y divide-line border-y border-line">
        {providers.map((provider) => (
          <div key={provider.id} className="flex items-center gap-3 py-4 text-sm">
            <div className="min-w-0 flex-1">
              <strong>{provider.name}</strong>
              <p className="truncate text-muted">
                {provider.model_id} · {provider.capability === "image" ? "图像" : "文本"} ·{" "}
                {provider.adapter === "jimeng" ? "即梦" : "OpenAI 兼容"} ·{" "}
                {provider.api_key_configured ? "密钥已配置" : "缺少密钥"}
              </p>
            </div>
            <button type="button" className="button-secondary" onClick={() => edit(provider)}>
              编辑
            </button>
            <button
              type="button"
              title="删除模型"
              aria-label={`删除${provider.name}`}
              onClick={() => remove(provider.id)}
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
      <form onSubmit={save} className="mt-7 grid gap-3 sm:grid-cols-2">
        <label className="text-sm">
          标识
          <input
            className="field mt-2"
            required
            disabled={editingId !== null}
            pattern="[A-Z0-9_]{2,40}"
            value={draft.id}
            onChange={(event) => setDraft({ ...draft, id: event.target.value.toUpperCase() })}
          />
        </label>
        <label className="text-sm">
          名称
          <input
            className="field mt-2"
            required
            value={draft.name}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />
        </label>
        <label className="text-sm">
          协议
          <select
            className="field mt-2"
            value={draft.adapter}
            onChange={(event) =>
              setDraft({
                ...draft,
                adapter: event.target.value as AgentProvider["adapter"],
                base_url: event.target.value === "jimeng" ? "" : "https://api.openai.com/v1",
                capability: event.target.value === "jimeng" ? "image" : draft.capability,
              })
            }
          >
            <option value="openai_compatible">OpenAI 兼容（OpenAI / DeepSeek）</option>
            <option value="jimeng">即梦</option>
          </select>
        </label>
        <label className="text-sm">
          能力
          <select
            className="field mt-2"
            value={draft.capability}
            onChange={(event) =>
              setDraft({ ...draft, capability: event.target.value as AgentProvider["capability"] })
            }
          >
            <option value="text">文本</option>
            <option value="image">图像</option>
          </select>
        </label>
        <label className="text-sm">
          API 基础地址
          <input
            className="field mt-2"
            type="url"
            required
            value={draft.base_url}
            onChange={(event) => setDraft({ ...draft, base_url: event.target.value })}
          />
        </label>
        <label className="text-sm">
          模型 ID
          <input
            className="field mt-2"
            required
            placeholder="提供商实际模型 ID"
            value={draft.model_id}
            onChange={(event) => setDraft({ ...draft, model_id: event.target.value })}
          />
        </label>
        <label className="text-sm">
          API Key {editingId && "（留空保留原值）"}
          <input
            className="field mt-2"
            type="password"
            autoComplete="off"
            required={!editingId}
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
          />
        </label>
        <label className="flex items-center gap-2 self-end pb-3 text-sm">
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })}
          />
          启用
        </label>
        <div className="sm:col-span-2">
          <button type="submit" className="button-primary">
            <Plus size={16} />
            {editingId ? "保存模型" : "添加模型"}
          </button>
          {editingId && (
            <button
              type="button"
              className="button-secondary ml-2"
              onClick={() => {
                setEditingId(null);
                setDraft(empty);
                setApiKey("");
              }}
            >
              取消编辑
            </button>
          )}
        </div>
      </form>
      <div className="mt-10 grid gap-4 sm:grid-cols-2">
        {tasks.map((task) => (
          <label key={task.id} className="text-sm">
            {task.name}模型
            <select
              className="field mt-2"
              value={bindings.find((item) => item.task === task.id)?.provider_id ?? ""}
              onChange={(event) => bind(task.id, event.target.value)}
            >
              <option value="">未绑定</option>
              {providers
                .filter((provider) => provider.enabled && provider.capability === task.capability)
                .map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.name} · {provider.model_id}
                  </option>
                ))}
            </select>
          </label>
        ))}
      </div>
      <p role="status" className="mt-4 text-sm text-muted">
        {message}
      </p>
    </section>
  );
}
