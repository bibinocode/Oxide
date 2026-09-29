import { useEffect } from "react";
import type { RefObject } from "react";

/** 仅在浏览器中标记视口下方内容；无 JS 与减少动画设置始终可读。 */
export function useScrollReveal(ref: RefObject<HTMLElement | null>, dependency?: unknown) {
  useEffect(() => {
    const root = ref.current;
    if (
      !root ||
      !window.IntersectionObserver ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    )
      return;
    const items = [...root.children].filter(
      (node): node is HTMLElement =>
        node instanceof HTMLElement && node.getBoundingClientRect().top > window.innerHeight * 0.92,
    );
    if (!items.length) return;
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;
          const element = entry.target as HTMLElement;
          element.classList.remove("reveal-pending");
          element.classList.add("reveal-in");
          observer.unobserve(element);
        });
      },
      { threshold: 0.05, rootMargin: "0px 0px 40px 0px" },
    );
    items.forEach((item) => {
      item.classList.add("reveal-pending");
      observer.observe(item);
    });
    return () => {
      observer.disconnect();
      items.forEach((item) => item.classList.remove("reveal-pending"));
    };
  }, [ref, dependency]);
}
