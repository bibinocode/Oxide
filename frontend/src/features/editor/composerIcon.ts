/** 原生 CodeMirror 控件使用线性 SVG，与 React 侧栏的 Lucide 图标保持同一笔画规范。 */
export function composerIcon(
  name: "wand" | "globe" | "clip" | "arrow" | "close" | "stop" | "loader",
) {
  const paths = {
    wand: ["m15 4 5 5L8 21l-5-5Z", "m12 7 5 5", "M5 3v4M3 5h4M19 14v4M17 16h4"],
    globe: [
      "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0",
      "M3 12h18M12 3a18 18 0 0 0 0 18 18 18 0 0 0 0-18",
    ],
    clip: ["m21 11-9 9a6 6 0 0 1-8-8L14 2a4 4 0 0 1 6 6L10 18a2 2 0 0 1-3-3l9-9"],
    arrow: ["M12 19V5m-6 6 6-6 6 6"],
    close: ["m6 6 12 12M6 18 18 6"],
    stop: ["M7 7h10v10H7Z"],
    loader: ["M21 12a9 9 0 1 1-9-9"],
  };
  const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  for (const [key, value] of Object.entries({
    viewBox: "0 0 24 24",
    width: "16",
    height: "16",
    fill: "none",
    stroke: "currentColor",
    "stroke-width": "1.6",
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
    "aria-hidden": "true",
  }))
    icon.setAttribute(key, value);
  for (const data of paths[name]) {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", data);
    icon.append(path);
  }
  return icon;
}
