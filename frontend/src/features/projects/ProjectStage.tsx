import { lazy, Suspense, useCallback, useEffect, useRef, useState, type ReactNode } from "react";

const ProjectShaderField = lazy(() =>
  import("./ProjectShaderField").then((module) => ({ default: module.ProjectShaderField })),
);

/** 悬停或键盘聚焦时显示网格与图标中心定位线，离开后释放动态图形资源。 */
export function ProjectStage({ children }: { children: ReactNode }) {
  const root = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState<HTMLElement | null>(null);
  const [focused, setFocused] = useState<HTMLElement | null>(null);
  const [supported, setSupported] = useState(false);
  const [motion, setMotion] = useState(false);
  const [ready, setReady] = useState(false);
  const target = hovered ?? focused;
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const onReady = useCallback(() => setReady(true), []);
  const onUnavailable = useCallback(() => {
    setSupported(false);
    setReady(false);
  }, []);

  useEffect(() => {
    const query = matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setMotion(!query.matches);
    sync();
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  }, []);

  useEffect(() => {
    if (!target || !motion) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const gpu = (navigator as Navigator & { gpu?: { requestAdapter: () => Promise<unknown> } }).gpu;
    if (!gpu) return;
    void Promise.race([
      gpu.requestAdapter(),
      new Promise<null>((resolve) => {
        timer = setTimeout(() => resolve(null), 1500);
      }),
    ])
      .then((adapter) => {
        if (!cancelled) setSupported(Boolean(adapter));
      })
      .catch(() => {
        if (!cancelled) setSupported(false);
      });
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [target, motion]);

  useEffect(() => {
    if (!target) return;
    const measure = () => {
      const parent = root.current?.getBoundingClientRect();
      const anchor = target.querySelector(".project-icon-frame")?.getBoundingClientRect();
      if (parent && anchor)
        setPosition({
          x: anchor.x - parent.x + anchor.width / 2,
          y: anchor.y - parent.y + anchor.height / 2,
        });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(target);
    if (root.current) observer.observe(root.current);
    return () => observer.disconnect();
  }, [target]);

  /** 只认项目行，避免列表间隙和触屏滚动触发悬停背景。 */
  function row(element: EventTarget | null): HTMLElement | null {
    return element instanceof Element ? element.closest<HTMLElement>(".project-row") : null;
  }
  return (
    <div
      ref={root}
      className="project-stage mt-10"
      data-active={Boolean(target)}
      onPointerOver={(event) => {
        if (
          event.pointerType === "mouse" &&
          matchMedia("(hover: hover) and (pointer: fine)").matches
        )
          setHovered(row(event.target));
      }}
      onPointerLeave={() => {
        setHovered(null);
        setReady(false);
      }}
      onFocusCapture={(event) => {
        if (event.target.matches(":focus-visible")) setFocused(row(event.target));
      }}
      onBlurCapture={(event) => {
        setFocused(row(event.relatedTarget));
        setReady(false);
      }}
    >
      <div className="project-stage-field" aria-hidden="true">
        <span
          className="project-blueprint-static"
          data-hidden={Boolean(target && ready && motion && supported)}
        />
        {target && motion && supported && (
          <Suspense fallback={null}>
            <ProjectShaderField onReady={onReady} onUnavailable={onUnavailable} />
          </Suspense>
        )}
        <svg
          className="project-registration"
          style={{ left: position.x, top: position.y }}
          viewBox="0 0 20 20"
          fill="none"
        >
          <circle cx="10" cy="10" r="7.4" />
          <path d="M10 0v5M10 15v5M0 10h5M15 10h5" />
        </svg>
      </div>
      {children}
    </div>
  );
}
