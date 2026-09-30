import { Globe, ExternalLink } from "lucide-react";
import { useId, useState } from "react";
import type { WritingProgress } from "../writingStream";

/** 同一回复的搜索与网页读取来源默认展示三条；展开保留服务端来源编号。 */
export function WritingSources({ progress }: { progress: WritingProgress[] }) {
  const [expanded, setExpanded] = useState(false);
  const listId = useId();
  const sources = progress.flatMap((entry) =>
    (entry.sources ?? []).flatMap((source) => {
      try {
        const url = new URL(source.url);
        return ["http:", "https:"].includes(url.protocol)
          ? [{ ...source, hostname: url.hostname, key: entry.id + ":" + source.reference }]
          : [];
      } catch {
        return [];
      }
    }),
  );
  if (!sources.length) return null;
  return (
    <section className="writing-sources" aria-label="联网参考来源">
      <h4>
        <Globe size={14} /> 参考来源
      </h4>
      <ol id={listId} className={expanded ? "writing-sources-expanded" : undefined}>
        {(expanded ? sources : sources.slice(0, 3)).map((source) => (
          <li key={source.key}>
            <a href={source.url} target="_blank" rel="noopener noreferrer">
              <span className="writing-source-index">{source.reference}</span>
              <span className="writing-source-title" title={source.title || source.url}>
                {source.title || source.url}
              </span>
              <ExternalLink size={12} />
            </a>
            <p>
              {source.source || source.hostname}
              {source.publish_date ? " · " + source.publish_date : ""}
            </p>
          </li>
        ))}
      </ol>
      {sources.length > 3 && (
        <button
          type="button"
          className="writing-sources-toggle"
          aria-expanded={expanded}
          aria-controls={listId}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? "收起来源" : `展开其余 ${sources.length - 3} 条来源`} · 共 {sources.length} 条
        </button>
      )}
    </section>
  );
}
