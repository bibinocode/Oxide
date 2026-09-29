import { Plus, Trash2 } from "lucide-react";
import type { SitePresentation, SiteSection } from "../../../lib/api/types";

/** 公开模块使用结构化表单维护，开关与内容分别保存。 */
export function SiteModulesEditor({
  value,
  onChange,
}: {
  value: SitePresentation;
  onChange: (value: SitePresentation) => void;
}) {
  return (
    <div className="space-y-8 border-t border-line pt-6">
      <label className="block text-sm">
        页脚署名
        <input
          className="field mt-2"
          maxLength={200}
          value={value.footer_text}
          placeholder="留空时使用站点名称"
          onChange={(e) => onChange({ ...value, footer_text: e.target.value })}
        />
      </label>
      <SectionEditor
        title="联系方式"
        value={value.contacts}
        onChange={(contacts) => onChange({ ...value, contacts })}
      />
      <SectionEditor
        title="项目作品集"
        value={value.projects}
        onChange={(projects) => onChange({ ...value, projects })}
      />
      <SectionEditor
        title="服务模块"
        value={value.services}
        onChange={(services) => onChange({ ...value, services })}
      />
    </div>
  );
}

/** 数组顺序即公开顺序，关闭模块时保留其全部条目。 */
function SectionEditor({
  title,
  value,
  onChange,
}: {
  title: string;
  value: SiteSection;
  onChange: (value: SiteSection) => void;
}) {
  return (
    <fieldset className="space-y-4">
      <legend className="mb-3 text-sm font-semibold">{title}</legend>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={value.enabled}
          onChange={(e) => onChange({ ...value, enabled: e.target.checked })}
        />
        在站点展示
      </label>
      {value.items.map((item, index) => (
        <div key={index} className="space-y-2 border border-line p-3">
          <div className="flex gap-2">
            <input
              aria-label={`${title}名称 ${index + 1}`}
              className="field"
              placeholder="名称"
              required
              maxLength={80}
              value={item.title}
              onChange={(e) =>
                onChange({
                  ...value,
                  items: value.items.map((old, i) =>
                    i === index ? { ...old, title: e.target.value } : old,
                  ),
                })
              }
            />
            <button
              type="button"
              aria-label={`删除${title} ${index + 1}`}
              onClick={() =>
                onChange({ ...value, items: value.items.filter((_, i) => i !== index) })
              }
            >
              <Trash2 size={16} />
            </button>
          </div>
          <input
            aria-label={`${title}链接 ${index + 1}`}
            className="field"
            placeholder="https:// 或 mailto: 或 /站内路径"
            required
            maxLength={2048}
            value={item.url}
            onChange={(e) =>
              onChange({
                ...value,
                items: value.items.map((old, i) =>
                  i === index ? { ...old, url: e.target.value } : old,
                ),
              })
            }
          />
          {title !== "联系方式" && (
            <input
              aria-label={`${title}说明 ${index + 1}`}
              className="field"
              placeholder="简短说明（选填）"
              maxLength={500}
              value={item.description}
              onChange={(e) =>
                onChange({
                  ...value,
                  items: value.items.map((old, i) =>
                    i === index ? { ...old, description: e.target.value } : old,
                  ),
                })
              }
            />
          )}
        </div>
      ))}
      <button
        type="button"
        disabled={value.items.length >= 30}
        className="button-secondary text-sm"
        onClick={() =>
          onChange({ ...value, items: [...value.items, { title: "", url: "", description: "" }] })
        }
      >
        <Plus size={15} />
        添加{title}
      </button>
    </fieldset>
  );
}
