import type { SiteSection as Section } from "../../lib/api/types";
import { SectionTag } from "./PrintMarks";

/** 联系方式、作品集与服务按管理端顺序展示；关闭时不输出占位。 */
export function SiteSection({
  id,
  title,
  index,
  section,
}: {
  id: string;
  title: string;
  index: string;
  section?: Section;
}) {
  if (!section?.enabled || !section.items.length) return null;
  return (
    <section id={id} className="mt-16 scroll-mt-8">
      <SectionTag index={index}>{title}</SectionTag>
      <div className="mt-4 border-t border-line">
        {section.items.map((item, i) => (
          <a
            key={i}
            href={item.url}
            rel="noopener noreferrer"
            className="block border-b border-line py-4"
          >
            <div className="flex items-center justify-between text-sm font-medium">
              <span>{item.title}</span>
              <span aria-hidden="true" className="text-muted">
                ↗
              </span>
            </div>
            {item.description && (
              <p className="mt-2 text-sm leading-6 text-muted">{item.description}</p>
            )}
          </a>
        ))}
      </div>
    </section>
  );
}
