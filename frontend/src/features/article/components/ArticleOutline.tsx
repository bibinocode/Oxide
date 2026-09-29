import { useEffect, useState } from "react";
import { ArrowLeft, ArrowUp, ListTree, X } from "lucide-react";
import { Link } from "@tanstack/react-router";
import { useArticleOutline } from "../hooks/useArticleOutline";

/** 文章地图：桌面固定侧栏，窄屏按钮展开；只在存在正文小节时显示。 */
export function ArticleOutline({
  bodyId,
  html,
  title,
}: {
  bodyId: string;
  html: string;
  title: string;
}) {
  const { items, activeId, progress } = useArticleOutline(bodyId, html, title);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const query = window.matchMedia("(min-width: 1100px)");
    const update = () => setOpen(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  if (items.length < 2) return null;

  function visit(event: React.MouseEvent<HTMLAnchorElement>, id: string) {
    event.preventDefault();
    const target = document.getElementById(id);
    if (!target) return;
    target.tabIndex = -1;
    target.focus({ preventScroll: true });
    target.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "start",
    });
    window.history.replaceState(null, "", `#${id}`);
    if (window.innerWidth < 1100) setOpen(false);
  }

  return (
    <div className="article-outline" data-open={open || undefined}>
      <button
        type="button"
        className="article-outline-toggle"
        title={open ? "收起文章目录" : "展开文章目录"}
        aria-label={open ? "收起文章目录" : "展开文章目录"}
        aria-expanded={open}
        aria-controls="article-outline-nav"
        onClick={() => setOpen((value) => !value)}
      >
        <svg className="article-outline-progress" viewBox="0 0 28 28" aria-hidden="true">
          <circle cx="14" cy="14" r="12" className="article-outline-track" />
          <circle
            cx="14"
            cy="14"
            r="12"
            className="article-outline-value"
            strokeDasharray={`${progress * 75.4} 75.4`}
          />
        </svg>
        {open ? <X size={15} /> : <ListTree size={15} />}
      </button>
      <nav
        id="article-outline-nav"
        className="article-outline-panel"
        aria-label="文章目录"
        hidden={!open}
      >
        <Link to="/archive" search={{ page: 1 }} className="article-outline-utility">
          <ArrowLeft size={13} /> 写作
        </Link>
        <ol className="article-outline-list">
          {items.map((item) => (
            <li key={item.id} data-level={item.level}>
              <a
                href={`#${item.id}`}
                aria-current={activeId === item.id ? "location" : undefined}
                title={item.label}
                onClick={(event) => visit(event, item.id)}
              >
                <span className="article-outline-tick" aria-hidden="true" />
                <span className="article-outline-label">{item.label}</span>
              </a>
            </li>
          ))}
        </ol>
        <button
          type="button"
          className="article-outline-utility"
          onClick={() => window.scrollTo({ top: 0, behavior: "smooth" })}
        >
          <ArrowUp size={13} /> 顶部
        </button>
      </nav>
    </div>
  );
}
