import { useEffect, type RefObject } from "react";

/** 连续按 G 与导航字母；编辑文字、组合输入或弹窗内不拦截键盘。 */
export function useDockShortcuts(dock: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    let armedAt: number | null = null;
    const reset = () => {
      armedAt = null;
    };
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.isComposing ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        event.shiftKey ||
        (target instanceof Element &&
          target.closest(
            'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"], [role="dialog"]',
          )) ||
        document.querySelector('[popover]:popover-open, dialog[open], [aria-modal="true"]')
      ) {
        reset();
        return;
      }
      const key = event.key.toUpperCase();
      if (armedAt !== null && performance.now() - armedAt < 1000) {
        reset();
        const link = Array.from(
          dock.current?.querySelectorAll<HTMLAnchorElement>("a[data-go-key]") ?? [],
        ).find((item) => item.dataset.goKey === key);
        if (link) {
          event.preventDefault();
          link.click();
          return;
        }
      }
      armedAt = key === "G" ? performance.now() : null;
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("blur", reset);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", reset);
    };
  }, [dock]);
}
