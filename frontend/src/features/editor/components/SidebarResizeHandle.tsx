import { useEffect, useRef, useState } from "react";

/** 右侧面板从左边缘调整宽度，始终给正文保留至少 320px；支持键盘和触摸。 */
export function SidebarResizeHandle({
  panel,
  value,
  onChange,
}: {
  panel: React.RefObject<HTMLElement | null>;
  value: number;
  onChange: (width: number) => void;
}) {
  const [maximum, setMaximum] = useState(800);
  const drag = useRef<{ x: number; width: number } | null>(null);
  useEffect(() => {
    const container = panel.current?.parentElement;
    if (!container) return;
    const observer = new ResizeObserver(() =>
      setMaximum(Math.max(320, Math.min(800, container.clientWidth - 320))),
    );
    observer.observe(container);
    return () => observer.disconnect();
  }, [panel]);
  const width = Math.max(320, Math.min(maximum, value));
  return (
    <div
      className="writing-sidebar-resize"
      role="separator"
      tabIndex={0}
      aria-label="调整 AI 侧栏宽度"
      aria-orientation="vertical"
      aria-valuemin={320}
      aria-valuemax={maximum}
      aria-valuenow={Math.round(width)}
      onPointerDown={(event) => {
        event.preventDefault();
        drag.current = {
          x: event.clientX,
          width: panel.current?.getBoundingClientRect().width ?? width,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (!drag.current || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
        onChange(
          Math.max(320, Math.min(maximum, drag.current.width + drag.current.x - event.clientX)),
        );
      }}
      onPointerUp={(event) => {
        drag.current = null;
        if (event.currentTarget.hasPointerCapture(event.pointerId))
          event.currentTarget.releasePointerCapture(event.pointerId);
      }}
      onPointerCancel={() => {
        drag.current = null;
      }}
      onDoubleClick={() => onChange(Math.min(400, maximum))}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
          event.preventDefault();
          onChange(
            Math.max(320, Math.min(maximum, width + (event.key === "ArrowLeft" ? 20 : -20))),
          );
        }
        if (event.key === "Home" || event.key === "End") {
          event.preventDefault();
          onChange(event.key === "Home" ? 320 : maximum);
        }
      }}
    />
  );
}
