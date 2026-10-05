import { useRef, useState } from "react";
import type { Taxonomy } from "../../../lib/api/types";

/** 标签与分类的隐藏可恢复；删除只移除关联，不删除文章。 */
export function TaxonomyEditor({
  title,
  kind,
  items,
  onCreate,
  onRemove,
  onVisibility,
}: {
  title: string;
  kind: "categories" | "tags";
  items: Taxonomy[];
  onCreate: (kind: "categories" | "tags", item: Taxonomy) => Promise<boolean>;
  onRemove: (kind: "categories" | "tags", slug: string) => Promise<boolean>;
  onVisibility: (kind: "categories" | "tags", item: Taxonomy) => Promise<boolean>;
}) {
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [pending, setPending] = useState(false);
  const lock = useRef(false);
  async function run(action: () => Promise<boolean>) {
    if (lock.current) return;
    lock.current = true;
    setPending(true);
    try {
      return await action();
    } finally {
      lock.current = false;
      setPending(false);
    }
  }
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (await run(() => onCreate(kind, { name, slug, visible: true }))) {
      setName("");
      setSlug("");
    }
  }
  return (
    <div>
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="mt-2 text-xs leading-6 text-muted">
        隐藏可恢复；删除只移除文章关联，文章会保留。
      </p>
      <div className="mt-5 border-t border-line">
        {items.map((item) => (
          <div
            key={item.slug}
            className="flex flex-wrap items-center justify-between gap-3 border-b border-line py-3 text-sm"
          >
            <span>
              {item.name}{" "}
              <span className="ml-2 text-muted">
                /{item.slug} · {item.visible ? "公开" : "已隐藏"}
              </span>
            </span>
            <div className="flex gap-2">
              <button
                type="button"
                disabled={pending}
                onClick={() => void run(() => onVisibility(kind, item))}
                className="button-secondary"
              >
                {item.visible ? "隐藏" : "恢复展示"}
              </button>
              <button
                type="button"
                disabled={pending}
                onClick={() => void run(() => onRemove(kind, item.slug))}
                className="button-secondary text-warm"
                aria-label={`删除${item.name}`}
              >
                删除
              </button>
            </div>
          </div>
        ))}
      </div>
      <form onSubmit={submit} className="mt-4 grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
        <input
          className="field"
          placeholder="名称"
          aria-label={`${title}名称`}
          required
          disabled={pending}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <input
          className="field"
          placeholder="slug"
          aria-label={`${title} slug`}
          required
          disabled={pending}
          value={slug}
          onChange={(event) => setSlug(event.target.value.toLowerCase())}
        />
        <button type="submit" disabled={pending} className="button-secondary">
          {pending ? "处理中…" : `添加${title}`}
        </button>
      </form>
    </div>
  );
}
