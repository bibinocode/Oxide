import { useEffect, useRef, useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { BookOpen, Plus, Save } from "lucide-react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";
import type { ColumnSummary } from "../lib/api/types";
import { ColumnContents } from "../features/admin/columns/ColumnContents";

export const Route = createFileRoute("/admin/columns")({ component: AdminColumns });

/** 专栏价格保留分单位，历史订单的价格快照由后端维护。 */
function AdminColumns() {
  const { session } = useAdminSession();
  const [columns, setColumns] = useState<ColumnSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [price, setPrice] = useState("");
  const [pending, setPending] = useState(false);
  const [loading, setLoading] = useState(true);
  // 同步锁阻止同一帧内重复提交，避免状态更新前发出两次新增请求。
  const saving = useRef(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    apiRequest<ColumnSummary[]>("/api/v1/admin/columns", { signal: controller.signal })
      .then((items) => {
        if (!controller.signal.aborted) setColumns(items);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted)
          setMessage(cause instanceof Error ? cause.message : "小册加载失败");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, []);

  function choose(column?: ColumnSummary) {
    setSelected(column?.public_id ?? null);
    setTitle(column?.title ?? "");
    setDescription(column?.description ?? "");
    setPrice(column ? (column.price_cents / 100).toFixed(2) : "");
    setMessage("");
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!session || loading || saving.current) return;
    saving.current = true;
    // 请求期间固定编辑对象；新建成功后继续新建，编辑成功后仍保留编辑对象。
    const selectedId = selected;
    setPending(true);
    setMessage("");
    try {
      const amount = Number(price);
      if (
        !Number.isFinite(amount) ||
        amount < 1 ||
        amount > 10000 ||
        !/^\d+(?:\.\d{1,2})?$/.test(price)
      ) {
        throw new Error("价格须为 1 至 10000 元，最多两位小数");
      }
      const value = await apiRequest<ColumnSummary>(
        selectedId ? `/api/v1/admin/columns/${selectedId}` : "/api/v1/admin/columns",
        {
          method: selectedId ? "PUT" : "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({ title, description, price_cents: Math.round(amount * 100) }),
        },
      );
      setColumns((items) =>
        selectedId
          ? items.map((item) => (item.public_id === selectedId ? value : item))
          : [value, ...items],
      );
      choose(selectedId ? value : undefined);
      setMessage(selectedId ? "小册修改已保存" : "小册已新增，可继续添加下一本");
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : "保存失败");
    } finally {
      saving.current = false;
      setPending(false);
    }
  }

  async function manage(action: "visibility" | "delete") {
    if (!selected || !session || saving.current || loading) return;
    const column = columns.find((item) => item.public_id === selected);
    if (!column) return;
    if (
      action === "delete" &&
      !window.confirm(`永久删除「${column.title}」？有文章、订单或订阅的小册不能删除，可使用下架。`)
    )
      return;
    saving.current = true;
    setPending(true);
    setMessage("");
    try {
      if (action === "delete") {
        await apiRequest(`/api/v1/admin/columns/${selected}`, {
          method: "DELETE",
          headers: csrfHeaders(session),
        });
        setColumns((items) => items.filter((item) => item.public_id !== selected));
        choose();
        setMessage("小册已删除");
      } else {
        const value = await apiRequest<ColumnSummary>(`/api/v1/admin/columns/${selected}`, {
          method: "PATCH",
          headers: csrfHeaders(session),
          body: JSON.stringify({ visible: !column.visible }),
        });
        setColumns((items) => items.map((item) => (item.public_id === selected ? value : item)));
        setMessage(
          value.visible ? "小册已重新上架" : "小册已下架，停止公开展示及新购买，已有订阅保留",
        );
      }
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : "操作失败");
    } finally {
      saving.current = false;
      setPending(false);
    }
  }

  return (
    <section className="max-w-4xl">
      <header className="flex items-center justify-between border-b border-line pb-5">
        <div>
          <p className="eyebrow">Collections</p>
          <h1 className="mt-2 text-xl font-semibold">小册</h1>
        </div>
        <button
          type="button"
          className="button-secondary"
          disabled={pending || loading}
          onClick={() => choose()}
        >
          <Plus size={15} /> 新建小册
        </button>
      </header>
      <div className="grid gap-10 py-8 lg:grid-cols-[220px_minmax(0,1fr)]">
        <nav aria-label="已有小册" className="border-t border-line">
          {columns.map((column) => (
            <button
              type="button"
              key={column.public_id}
              disabled={pending || loading}
              aria-current={selected === column.public_id ? "page" : undefined}
              className="flex w-full items-center gap-2 border-b border-line py-3 text-left text-sm"
              onClick={() => choose(column)}
            >
              <BookOpen size={15} className="shrink-0 text-muted" />
              <span className="min-w-0 truncate">
                {column.title}
                {!column.visible && " · 已下架"}
              </span>
            </button>
          ))}
          {loading ? (
            <p className="py-4 text-sm text-muted">正在加载小册…</p>
          ) : (
            columns.length === 0 && <p className="py-4 text-sm text-muted">暂无小册</p>
          )}
        </nav>
        <div className="min-w-0">
          <form onSubmit={save} className="space-y-5">
            <h2 className="text-base font-semibold">{selected ? "编辑小册" : "新建小册"}</h2>
            <label className="block text-sm">
              小册名称
              <input
                className="field mt-2"
                required
                disabled={pending || loading}
                maxLength={160}
                value={title}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label className="block text-sm">
              简介
              <textarea
                className="field mt-2 min-h-28"
                maxLength={2000}
                disabled={pending || loading}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
              />
            </label>
            <label className="block text-sm">
              价格（元）
              <input
                className="field mt-2"
                required
                type="number"
                disabled={pending || loading}
                min="1"
                max="10000"
                step="0.01"
                value={price}
                onChange={(event) => setPrice(event.target.value)}
              />
            </label>
            <button
              type="submit"
              className="button-primary"
              disabled={pending || loading || !session}
            >
              <Save size={15} /> {pending ? "正在保存…" : selected ? "保存修改" : "添加小册"}
            </button>
            {selected && (
              <div className="flex flex-wrap gap-3 border-t border-line pt-4">
                <button
                  type="button"
                  className="button-secondary"
                  disabled={pending || loading}
                  onClick={() => void manage("visibility")}
                >
                  {columns.find((item) => item.public_id === selected)?.visible
                    ? "下架 / 隐藏小册"
                    : "重新上架"}
                </button>
                <button
                  type="button"
                  className="button-secondary text-warm"
                  disabled={pending || loading}
                  onClick={() => void manage("delete")}
                >
                  删除小册
                </button>
              </div>
            )}
            {message && (
              <p role="status" className="text-sm text-muted">
                {message}
              </p>
            )}
          </form>
          {selected ? (
            <ColumnContents key={selected} columnId={selected} />
          ) : (
            <p className="mt-8 text-sm text-muted">选择左侧已有小册，即可查看它的文章目录。</p>
          )}
        </div>
      </div>
    </section>
  );
}
