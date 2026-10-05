import { useEffect, useState } from "react";

const height = 64;
const rise = 40;

/** 根据视口宽度绘制弧形标尺：48px 主刻度、12px 副刻度，共用中心对齐点。 */
function RulerArc({ width, edge }: { width: number; edge: "top" | "bottom" }) {
  // 使用弦长和弓高计算圆半径，窄屏也维持相同的边缘抬升量。
  const radius = (width * width) / (8 * rise) + rise / 2;
  const apex = edge === "top" ? 20.5 : height - 8.5;
  const chordY = apex + (edge === "top" ? -rise : rise);
  const path = `M 0 ${chordY} A ${radius} ${radius} 0 0 ${edge === "top" ? 0 : 1} ${width} ${chordY}`;
  const offset = -(width / 2 - 0.5);
  return (
    <svg width={width} height={height} className={`page-ruler page-ruler-${edge}`}>
      <path
        d={path}
        pathLength={width}
        fill="none"
        className="page-ruler-major"
        strokeWidth={5}
        strokeDasharray="1 47"
        strokeDashoffset={offset}
      />
      <path
        d={path}
        pathLength={width}
        fill="none"
        className="page-ruler-minor"
        strokeWidth={2.5}
        strokeDasharray="1 11"
        strokeDashoffset={offset}
        transform={`translate(0 ${edge === "top" ? -1.25 : 1.25})`}
      />
    </svg>
  );
}

/** 公开页的视口装饰；挂载后测量以保持 SSR 一致，卸载时清理尺寸监听。 */
export function PageRulers() {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const measure = () => setWidth(document.documentElement.clientWidth);
    const observer = new ResizeObserver(measure);
    measure();
    observer.observe(document.documentElement);
    return () => observer.disconnect();
  }, []);
  if (!width) return null;
  return (
    <div className="page-rulers" aria-hidden="true">
      <RulerArc width={width} edge="top" />
      <RulerArc width={width} edge="bottom" />
    </div>
  );
}
