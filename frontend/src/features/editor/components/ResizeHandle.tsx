import type { PointerEvent } from "react";

/** 指针捕获兼容鼠标和触摸，方向键提供等价的分栏调整。 */
export function ResizeHandle({
  label,
  value,
  onChange,
  container,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  container: React.RefObject<HTMLDivElement | null>;
}) {
  function move(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    const bounds = container.current?.getBoundingClientRect();
    if (bounds?.width)
      onChange(Math.max(20, Math.min(80, ((event.clientX - bounds.left) / bounds.width) * 100)));
  }
  return (
    <div
      className="editor-resize-handle"
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemin={20}
      aria-valuemax={80}
      aria-valuenow={Math.round(value)}
      onPointerDown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={move}
      onPointerUp={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId))
          event.currentTarget.releasePointerCapture(event.pointerId);
      }}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
          event.preventDefault();
          onChange(Math.max(20, Math.min(80, value + (event.key === "ArrowRight" ? 2 : -2))));
        }
      }}
    />
  );
}
