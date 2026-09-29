import { useEffect, useState } from "react";

type Highlighter = Awaited<ReturnType<typeof import("shiki").createHighlighter>>;
let highlighterPromise: Promise<Highlighter> | null = null;

/** 语法文件按需加载并共享实例，正文没有代码时不下载高亮器。 */
function getHighlighter() {
  highlighterPromise ??= import("shiki")
    .then(({ createHighlighter }) =>
      createHighlighter({
        themes: ["github-light", "github-dark"],
        langs: [
          "bash",
          "css",
          "html",
          "javascript",
          "json",
          "markdown",
          "python",
          "rust",
          "sql",
          "typescript",
          "yaml",
          "toml",
        ],
      }),
    )
    .catch((error) => {
      highlighterPromise = null;
      throw error;
    });
  return highlighterPromise;
}

/** 公开页和编辑预览共享代码纸张、高亮及复制按钮，过期任务不能覆盖当前正文。 */
export function useArticleHtml(html: string) {
  const [enhanced, setEnhanced] = useState({ source: "", html: "" });
  useEffect(() => {
    if (!html.includes("<pre")) return;
    let cancelled = false;
    getHighlighter()
      .then((highlighter) => {
        if (cancelled) return;
        const document = new DOMParser().parseFromString(html, "text/html");
        for (const code of document.querySelectorAll("pre > code")) {
          const language =
            [...code.classList].find((name) => name.startsWith("language-"))?.slice(9) ?? "text";
          const lang = highlighter.getLoadedLanguages().includes(language) ? language : "text";
          const frame = document.createElement("figure");
          frame.className = "article-code-frame";
          const bar = document.createElement("figcaption");
          const label = document.createElement("span");
          label.textContent = language;
          const copy = document.createElement("button");
          copy.type = "button";
          copy.dataset.copyCode = "true";
          copy.textContent = "复制";
          copy.setAttribute("aria-label", "复制代码");
          bar.append(label, copy);
          frame.append(bar);
          const codeHtml = document.createElement("div");
          codeHtml.innerHTML = highlighter.codeToHtml(code.textContent ?? "", {
            lang,
            themes: { light: "github-light", dark: "github-dark" },
          });
          frame.append(...codeHtml.childNodes);
          code.parentElement?.replaceWith(frame);
        }
        if (!cancelled) setEnhanced({ source: html, html: document.body.innerHTML });
      })
      .catch(() => {
        /* 高亮加载失败时保留服务端提供的完整可读代码。 */
      });
    return () => {
      cancelled = true;
    };
  }, [html]);
  return enhanced.source === html ? enhanced.html : html;
}
