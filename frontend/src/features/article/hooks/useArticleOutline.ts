import { useEffect, useState } from "react";

export interface OutlineItem {
  id: string;
  label: string;
  level: 1 | 2 | 3;
}

/** 从正式文章 DOM 提取标题，并跟踪阅读位置；代码高亮重绘后会重新绑定标题。 */
export function useArticleOutline(bodyId: string, html: string, title: string) {
  const [items, setItems] = useState<OutlineItem[]>([]);
  const [activeId, setActiveId] = useState("article-start");
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    const body = document.getElementById(bodyId);
    const article = body?.closest(".article-reader");
    const start = article?.querySelector<HTMLElement>(".article-title-card h1");
    if (!body || !start) return;
    start.id = "article-start";
    let targets: HTMLElement[] = [];
    let frame = 0;
    let hashHandled = false;

    function measure() {
      frame = 0;
      let current = targets[0]?.id ?? "article-start";
      for (const target of targets) {
        if (target.getBoundingClientRect().top <= 120) current = target.id;
        else break;
      }
      const bottom = body!.getBoundingClientRect().bottom;
      if (window.scrollY <= 1) current = "article-start";
      else if (bottom <= window.innerHeight * 0.55) current = targets.at(-1)?.id ?? current;
      setActiveId(current);

      const top = start!.getBoundingClientRect().top + window.scrollY;
      const end = body!.getBoundingClientRect().bottom + window.scrollY - window.innerHeight * 0.55;
      setProgress(Math.max(0, Math.min(1, (window.scrollY - top) / Math.max(1, end - top))));
    }

    function scheduleMeasure() {
      if (!frame) frame = window.requestAnimationFrame(measure);
    }

    function rebuild() {
      const headings = Array.from(body!.querySelectorAll<HTMLElement>("h2, h3"));
      headings.forEach((heading, index) => {
        heading.id = `article-section-${index + 1}`;
      });
      targets = [start!, ...headings];
      setItems([
        { id: "article-start", label: title, level: 1 },
        ...headings.map((heading) => ({
          id: heading.id,
          label: heading.textContent?.trim() || "未命名小节",
          level: Number(heading.tagName.slice(1)) as 2 | 3,
        })),
      ]);
      scheduleMeasure();
      if (!hashHandled && /^#article-(start|section-\d+)$/.test(window.location.hash)) {
        hashHandled = true;
        window.requestAnimationFrame(() =>
          document.getElementById(window.location.hash.slice(1))?.scrollIntoView(),
        );
      }
    }

    rebuild();
    const observer = new MutationObserver(rebuild);
    observer.observe(body, { childList: true, subtree: true });
    window.addEventListener("scroll", scheduleMeasure, { passive: true });
    window.addEventListener("resize", scheduleMeasure);
    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", scheduleMeasure);
      window.removeEventListener("resize", scheduleMeasure);
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [bodyId, html, title]);

  return { items, activeId, progress };
}
