import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";

/** 粘贴富文本时保留常见文档结构，纯文本由 CodeMirror 原生处理。 */
export function htmlToMarkdown(html: string): string {
  const document = new DOMParser().parseFromString(html, "text/html");
  document
    .querySelectorAll(
      "script,style,iframe,object,embed,form,.article-link-icon,figure.article-code-frame > figcaption,figure.article-image > figcaption,button[data-copy-code]",
    )
    .forEach((node) => node.remove());
  const converter = new TurndownService({
    headingStyle: "atx",
    codeBlockStyle: "fenced",
    bulletListMarker: "-",
  });
  converter.use(gfm);
  converter.addRule("fencedCodeWithLanguage", {
    filter: (node) => node.nodeName === "PRE" && !!node.querySelector("code"),
    replacement: (_content, node) => {
      const code = (node as HTMLElement).querySelector("code")!;
      const language = code.className.match(/(?:language|lang)-([\w+-]+)/)?.[1] ?? "";
      const content = code.textContent?.replace(/\n$/, "") ?? "";
      const fence = "`".repeat(
        Math.max(3, ...[...content.matchAll(/`+/g)].map((match) => match[0].length + 1)),
      );
      return `\n\n${fence}${language}\n${content}\n${fence}\n\n`;
    },
  });
  return converter.turndown(document.body).trim();
}
