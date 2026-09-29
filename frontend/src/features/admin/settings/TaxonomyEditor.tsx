import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Taxonomy } from "../../../lib/api/types";

export function TaxonomyEditor({
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
