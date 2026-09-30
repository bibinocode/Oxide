import { Children, isValidElement, useEffect, useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import { getCodeHighlighter, codeLanguage } from "../../../lib/codeHighlighter";

/** 高亮结果绑定源码和语言；流式更新取消旧任务，失败时仍可阅读原始代码。 */
function WritingCodeBlock({ source, language }: { source: string; language: string }) {
  const [highlighted, setHighlighted] = useState<{
    source: string;
    language: string;
    html: string;
  } | null>(null);
  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(() => {
      void getCodeHighlighter()
        .then((highlighter) => {
          if (cancelled) return;
          const normalized = codeLanguage(language);
          const lang = highlighter.getLoadedLanguages().includes(normalized) ? normalized : "text";
          const html = highlighter.codeToHtml(source, {
            lang,
            themes: { light: "github-light", dark: "github-dark" },
          });
          setHighlighted({ source, language, html });
        })
        .catch(() => {
          /* 网络或语法加载失败时保留纯文本代码，不中断对话。 */
        });
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [source, language]);
  return (
    <figure className="writing-code-block">
      <figcaption>{language || "text"}</figcaption>
      {highlighted?.source === source && highlighted.language === language ? (
        <div dangerouslySetInnerHTML={{ __html: highlighted.html }} />
      ) : (
        <pre>
          <code>{source}</code>
        </pre>
      )}
    </figure>
  );
}

/** 在 pre 边界区分围栏代码与行内标识符，兼容新版 react-markdown 的组件接口。 */
const components: Components = {
  pre({ children }) {
    const code = Children.toArray(children).find(
      (child) =>
        isValidElement<{ children?: ReactNode; className?: string }>(child) &&
        child.type === "code",
    );
    if (!isValidElement<{ children?: ReactNode; className?: string }>(code))
      return <pre>{children}</pre>;
    const source = String(code.props.children ?? "").replace(/\n$/, "");
    const language = code.props.className?.match(/language-([\w+-]+)/)?.[1] ?? "text";
    return <WritingCodeBlock source={source} language={language} />;
  },
};

/** 对话 Markdown 沿用安全解析；只插入 Shiki 对代码转义后生成的 HTML。 */
export function WritingMarkdown({ content }: { content: string }) {
  return <ReactMarkdown components={components}>{content}</ReactMarkdown>;
}
