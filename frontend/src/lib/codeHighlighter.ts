type Highlighter = Awaited<ReturnType<typeof import("shiki").createHighlighter>>;
let highlighterPromise: Promise<Highlighter> | null = null;

/** 正文预览与 AI 对话共用按需加载的实例，避免重复加载语法和主题。 */
export function getCodeHighlighter() {
  highlighterPromise ??= import("shiki")
    .then(({ createHighlighter }) =>
      createHighlighter({
        themes: ["github-light", "github-dark"],
        langs: [
          "bash",
          "css",
          "html",
          "javascript",
          "jsx",
          "json",
          "markdown",
          "python",
          "rust",
          "sql",
          "typescript",
          "tsx",
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

/** 常用围栏别名标准化；未知语言保留纯文本，避免高亮失败隐藏代码。 */
export function codeLanguage(language: string) {
  const normalized = language.toLowerCase();
  const aliases: Record<string, string> = {
    js: "javascript",
    ts: "typescript",
    sh: "bash",
    shell: "bash",
    py: "python",
    rs: "rust",
    yml: "yaml",
    "react-jsx": "jsx",
    "react-tsx": "tsx",
  };
  return aliases[normalized] ?? normalized;
}
