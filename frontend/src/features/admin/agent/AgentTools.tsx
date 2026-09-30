import { useEffect, useRef, useState } from "react";
import { Globe, Save, Search } from "lucide-react";
import { csrfHeaders, useAdminSession } from "../AdminSession";
import { apiRequest } from "../../../lib/api/client";
import type {
  AgentToolDescriptor,
  WebSearchSettings,
  WebSearchResponse,
} from "../../../lib/api/types";

const endpoint = "/api/v1/admin/agent/tools";
const recencies = [
  ["noLimit", "不限时间"],
  ["oneDay", "一天内"],
  ["oneWeek", "一周内"],
  ["oneMonth", "一月内"],
  ["oneYear", "一年内"],
] as const;
function messageOf(error: unknown) {
  return error instanceof Error ? error.message : "操作失败，请重试";
}

/** 工具权限与调用参数独立配置；主动测试仅使用已保存的设置，不读取密钥。 */
export function AgentTools({ onDirtyChange }: { onDirtyChange: (dirty: boolean) => void }) {
  const { session } = useAdminSession();
  const [tool, setTool] = useState<AgentToolDescriptor | null>(null);
  const [draft, setDraft] = useState<WebSearchSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [reload, setReload] = useState(0);
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<WebSearchResponse | null>(null);
  const request = useRef<AbortController | null>(null);
  const dirty = !!draft && !!tool && JSON.stringify(draft) !== JSON.stringify(tool.settings);
  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(
    () => () => {
      onDirtyChange(false);
      request.current?.abort();
    },
    [onDirtyChange],
  );
  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError("");
    apiRequest<AgentToolDescriptor[]>(endpoint, { signal: controller.signal })
      .then((tools) => {
        if (controller.signal.aborted) return;
        const next = tools.find((item) => item.name === "webSearch");
        if (!next) throw new Error("webSearch 尚未注册");
        setTool(next);
        setDraft(next.settings);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted) setError(messageOf(cause));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [reload]);

  function update(patch: Partial<WebSearchSettings>) {
    setDraft((old) => (old ? { ...old, ...patch } : old));
    setNotice("");
    setResult(null);
  }
  async function save() {
    if (!draft || busy || !session) return;
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const settings = await apiRequest<WebSearchSettings>(endpoint + "/webSearch", {
        method: "PUT",
        headers: csrfHeaders(session),
        body: JSON.stringify(draft),
        signal: controller.signal,
      });
      if (controller.signal.aborted) return;
      setDraft(settings);
      setTool((old) => (old ? { ...old, settings } : old));
      setNotice("已保存，下次 Agent 请求立即生效。");
    } catch (cause) {
      if (!controller.signal.aborted) setError(messageOf(cause));
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  }
  async function test() {
    if (busy || dirty || !query.trim() || !session) return;
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    setError("");
    setNotice("");
    setResult(null);
    try {
      const next = await apiRequest<WebSearchResponse>(endpoint + "/webSearch/test", {
        method: "POST",
        headers: csrfHeaders(session),
        body: JSON.stringify({ query: query.trim() }),
        signal: controller.signal,
      });
      if (!controller.signal.aborted) setResult(next);
    } catch (cause) {
      if (!controller.signal.aborted) setError(messageOf(cause));
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  }
  if (loading)
    return (
      <p role="status" className="agent-empty">
        正在读取工具配置…
      </p>
    );
  if (!tool || !draft)
    return (
      <div className="agent-empty">
        <p role="alert">{error || "没有可用工具"}</p>
        <button className="button-secondary mt-4" onClick={() => setReload((old) => old + 1)}>
          重新加载
        </button>
      </div>
    );
  return (
    <div className="agent-tools-layout">
      <section className="agent-skill-editor">
        <header className="agent-skill-intro">
          <div>
            <h2 className="flex items-center gap-2 text-lg font-semibold">
              <Globe size={20} /> webSearch
            </h2>
            <p className="mt-2 text-sm text-muted">智谱网络搜索 · Agent 根据任务按需调用</p>
          </div>
          <span className="agent-label">
            {tool.configured ? "环境密钥已配置" : "环境密钥未配置"}
          </span>
        </header>
        <p className="mb-5 text-sm text-muted">
          无需安装 Skill。每轮最多搜索 3 次、读取 2 篇公开 HTTPS
          文章；网页内容是不可信资料，不作为指令执行。
        </p>
        {!tool.configured && (
          <p role="status" className="mb-4 text-sm">
            搜索需要在服务器设置 WEB_SEARCH_API_KEY 并重启 API；链接读取无需搜索密钥。
          </p>
        )}
        <fieldset disabled={busy} className="agent-tool-fields">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(event) => update({ enabled: event.target.checked })}
            />{" "}
            启用网络搜索与链接读取
          </label>
          <div>
            <p className="mb-2 text-sm font-medium">任务授权</p>
            <div className="flex flex-wrap gap-5">
              {[
                ["writing", "写作 / AI 对话"],
                ["summary", "文章摘要"],
              ].map(([task, label]) => (
                <label key={task} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={draft.tasks.includes(task)}
                    onChange={(event) =>
                      update({
                        tasks: event.target.checked
                          ? [...draft.tasks, task]
                          : draft.tasks.filter((item) => item !== task),
                      })
                    }
                  />
                  {label}
                </label>
              ))}
            </div>
            <p className="mt-2 text-xs text-muted">摘要默认不联网；授权后会增加耗时与搜索费用。</p>
          </div>
          <div className="agent-tool-pair">
            <label>
              搜索引擎
              <select
                value={draft.search_engine}
                onChange={(event) => {
                  const engine = event.target.value as WebSearchSettings["search_engine"];
                  update({
                    search_engine: engine,
                    ...(engine === "search_pro_sogou" ? { count: 10 } : {}),
                    ...(engine === "search_pro_quark" ? { search_domain_filter: null } : {}),
                  });
                }}
              >
                <option value="search_std">标准搜索</option>
                <option value="search_pro">高级搜索</option>
                <option value="search_pro_sogou">搜狗搜索</option>
                <option value="search_pro_quark">夸克搜索</option>
              </select>
            </label>
            <label>
              结果上限
              <input
                type="number"
                min={1}
                max={10}
                disabled={draft.search_engine === "search_pro_sogou"}
                value={draft.count}
                onChange={(event) => update({ count: Number(event.target.value) })}
              />
            </label>
            <label>
              默认时间范围
              <select
                value={draft.search_recency_filter}
                onChange={(event) =>
                  update({
                    search_recency_filter: event.target
                      .value as WebSearchSettings["search_recency_filter"],
                  })
                }
              >
                {recencies.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              摘要长度
              <select
                value={draft.content_size}
                onChange={(event) =>
                  update({ content_size: event.target.value as WebSearchSettings["content_size"] })
                }
              >
                <option value="medium">适中</option>
                <option value="high">详细</option>
              </select>
            </label>
          </div>
          <label>
            固定搜索域名
            <input
              placeholder="例如 www.rust-lang.org（可留空）"
              disabled={draft.search_engine === "search_pro_quark"}
              value={draft.search_domain_filter ?? ""}
              onChange={(event) => update({ search_domain_filter: event.target.value || null })}
            />
            <span className="text-xs text-muted">
              只填裸域名，不含协议或路径；固定后模型不能扩大搜索范围。
            </span>
          </label>
          <p className="text-xs text-muted">
            搜狗固定 10
            条；夸克不支持域名筛选，结果上限仅在本地截取。不同引擎按提供商规则计费，不自动重试。
          </p>
        </fieldset>
        <div className="mt-6 flex flex-wrap gap-3">
          <button
            className="button-primary"
            disabled={
              busy ||
              !dirty ||
              draft.tasks.length === 0 ||
              !Number.isInteger(draft.count) ||
              draft.count < 1 ||
              draft.count > 10
            }
            onClick={() => void save()}
          >
            <Save size={16} /> {busy ? "处理中…" : "保存配置"}
          </button>
          <button
            className="button-secondary"
            disabled={busy || !dirty}
            onClick={() => {
              setDraft(tool.settings);
              setError("");
              setNotice("");
            }}
          >
            放弃修改
          </button>
          {dirty && <span className="self-center text-xs text-muted">有未保存修改</span>}
        </div>
        {error && (
          <p role="alert" className="mt-4 text-sm">
            {error}
          </p>
        )}
        {notice && (
          <p role="status" className="mt-4 text-sm">
            {notice}
          </p>
        )}
      </section>
      <section className="agent-skill-editor">
        <h2 className="flex items-center gap-2 text-lg font-semibold">
          <Search size={20} /> 搜索测试
        </h2>
        <p className="mt-3 text-sm text-muted">
          主动调用真实搜索 API，可能计费。使用已保存设置，不调用聊天模型。
        </p>
        <form
          className="agent-tool-fields mt-5"
          onSubmit={(event) => {
            event.preventDefault();
            void test();
          }}
        >
          <label>
            公开搜索关键词
            <input
              value={query}
              disabled={busy}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="例如 Rust 官方文档"
            />
            <span className="text-xs text-muted">
              {Array.from(query.trim()).length} / 70 字符 · 请勿输入密钥或私人内容
            </span>
          </label>
          <button
            className="button-secondary"
            disabled={
              busy ||
              dirty ||
              !tool.configured ||
              !tool.settings.enabled ||
              !query.trim() ||
              Array.from(query.trim()).length > 70
            }
            type="submit"
          >
            {busy ? "处理中…" : "执行搜索"}
          </button>
          {dirty && <p className="text-xs text-muted">请先保存配置再测试。</p>}
        </form>
        {result && (
          <div className="mt-6" aria-live="polite">
            <p className="text-sm text-muted">
              找到 {result.results.length} 条来源
              {result.results.length === 0 ? "，请尝试其他关键词。" : ""}
            </p>
            <ul className="agent-search-results">
              {result.results.map((hit) => (
                <li key={hit.reference}>
                  <a
                    href={hit.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-medium"
                  >
                    {hit.title || hit.url}
                  </a>
                  <p className="mt-2 whitespace-pre-wrap text-sm">{hit.content}</p>
                  <p className="mt-3 text-xs text-muted">
                    {hit.source} {hit.publish_date}
                    {hit.truncated && " · 摘要已截断"}
                  </p>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>
    </div>
  );
}
