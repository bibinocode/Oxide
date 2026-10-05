import { useEffect, useRef, useState } from "react";
import { PortraitContourField } from "./PortraitContourField";

interface Dot {
  x: number;
  y: number;
  radius: number;
  /** 深色背景以亮部着墨，保持原图明暗关系而非生成负片。 */
  darkRadius: number;
}

/** 原图转为半调印刷网点，指针接近时让网点轻微膨胀、偏移。 */
export function HalftonePortrait({ src, alt }: { src: string; alt: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [ready, setReady] = useState(false);
  const [active, setActive] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);
  const touchTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setReducedMotion(preference.matches);
    sync();
    preference.addEventListener("change", sync);
    return () => preference.removeEventListener("change", sync);
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    const button = buttonRef.current;
    if (!canvas || !button || !src) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    let disposed = false;
    let dots: Dot[] = [];
    let frame = 0;
    const pointer = { x: -1000, y: -1000, intensity: 0 };
    const target = { x: -1000, y: -1000, intensity: 0 };
    const image = new Image();
    image.crossOrigin = "anonymous";

    function paint() {
      if (!canvas || !context) return;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height);
      const dark = document.documentElement.dataset.theme === "dark";
      context.fillStyle = getComputedStyle(document.documentElement).getPropertyValue(
        "--portrait-ink",
      );
      for (const dot of dots) {
        let { x, y } = dot;
        let radius = dark ? dot.darkRadius : dot.radius;
        if (radius < 0.25) continue;
        if (pointer.intensity > 0.01) {
          const dx = x - pointer.x;
          const dy = y - pointer.y;
          const distance = Math.hypot(dx, dy);
          if (distance < 150) {
            const strength = (1 - distance / 150) ** 2 * pointer.intensity;
            radius *= 1 + strength * 0.08;
            const push = (strength * 3) / Math.max(1, distance);
            x += dx * push;
            y += dy * push;
          }
        }
        context.beginPath();
        context.arc(x, y, radius, 0, Math.PI * 2);
        context.fill();
      }
    }

    function tick() {
      frame = 0;
      pointer.x += (target.x - pointer.x) * 0.2;
      pointer.y += (target.y - pointer.y) * 0.2;
      pointer.intensity += (target.intensity - pointer.intensity) * 0.16;
      paint();
      if (
        Math.abs(pointer.intensity - target.intensity) > 0.01 ||
        (target.intensity > 0 && Math.hypot(pointer.x - target.x, pointer.y - target.y) > 0.5)
      )
        frame = requestAnimationFrame(tick);
    }

    function wake() {
      if (!frame) frame = requestAnimationFrame(tick);
    }

    function build() {
      if (disposed || !canvas || !image.naturalWidth) return;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      if (!width || !height) return;
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
      const cell = width < 180 ? 2.4 : 3.2;
      const columns = Math.ceil(width / cell);
      const rows = Math.ceil(height / cell);
      const sample = document.createElement("canvas");
      sample.width = columns;
      sample.height = rows;
      const sampleContext = sample.getContext("2d", { willReadFrequently: true });
      if (!sampleContext) return;
      const scale = Math.max(columns / image.naturalWidth, rows / image.naturalHeight);
      const drawnWidth = image.naturalWidth * scale;
      const drawnHeight = image.naturalHeight * scale;
      sampleContext.drawImage(
        image,
        (columns - drawnWidth) / 2,
        (rows - drawnHeight) / 2,
        drawnWidth,
        drawnHeight,
      );
      let pixels: Uint8ClampedArray;
      try {
        pixels = sampleContext.getImageData(0, 0, columns, rows).data;
      } catch {
        return;
      }
      const luminances = new Float32Array(columns * rows);
      for (let index = 0; index < luminances.length; index++) {
        const offset = index * 4;
        luminances[index] =
          (pixels[offset] * 0.2126 + pixels[offset + 1] * 0.7152 + pixels[offset + 2] * 0.0722) /
          255;
      }
      const sorted = Float32Array.from(luminances).sort();
      const low = sorted[Math.floor(sorted.length * 0.05)];
      const range = Math.max(0.05, sorted[Math.floor(sorted.length * 0.95)] - low);
      dots = [];
      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < columns; x++) {
          const luminance = Math.min(1, Math.max(0, (luminances[y * columns + x] - low) / range));
          const px = (x + 0.5) * cell;
          const py = (y + 0.5) * cell;
          // 透明素材保留透明区域；深色模式同时柔化上边缘，避免浅色原图形成硬方框。
          const alpha = pixels[(y * columns + x) * 4 + 3] / 255;
          const edge = Math.min(1, px / 16, (width - px) / 16, (height - py) / 18) * alpha;
          const radius = Math.min(cell * 0.51, (1 - luminance) * edge * cell * 0.55);
          const darkRadius = Math.min(
            cell * 0.51,
            luminance * edge * Math.min(1, py / 18) * cell * 0.55,
          );
          if (Math.max(radius, darkRadius) >= 0.25) dots.push({ x: px, y: py, radius, darkRadius });
        }
      }
      paint();
      setReady(true);
    }

    function onMove(event: PointerEvent) {
      if (
        event.pointerType !== "mouse" ||
        window.matchMedia("(prefers-reduced-motion: reduce)").matches
      )
        return;
      const rect = canvas!.getBoundingClientRect();
      if (target.intensity === 0) {
        pointer.x = event.clientX - rect.left;
        pointer.y = event.clientY - rect.top;
      }
      target.x = event.clientX - rect.left;
      target.y = event.clientY - rect.top;
      target.intensity = 1;
      wake();
    }

    function onLeave() {
      target.intensity = 0;
      wake();
    }

    image.onload = build;
    image.src = src;
    if (image.complete && image.naturalWidth) build();
    const resize = new ResizeObserver(build);
    resize.observe(canvas);
    const theme = new MutationObserver(paint);
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    button.addEventListener("pointermove", onMove);
    button.addEventListener("pointerleave", onLeave);
    return () => {
      disposed = true;
      resize.disconnect();
      theme.disconnect();
      button.removeEventListener("pointermove", onMove);
      button.removeEventListener("pointerleave", onLeave);
      if (frame) cancelAnimationFrame(frame);
      image.onload = null;
    };
  }, [src]);

  useEffect(() => () => window.clearTimeout(touchTimer.current), []);

  return (
    <span className="home-portrait-stage" data-active={active}>
      <PortraitContourField active={active} motion={active && !reducedMotion} />
      <button
        ref={buttonRef}
        type="button"
        className="home-portrait-trigger"
        aria-label={alt || "个人肖像"}
        onPointerEnter={(event) => {
          if (event.pointerType === "mouse") setActive(true);
        }}
        onPointerLeave={(event) => {
          if (event.pointerType === "mouse") setActive(false);
        }}
        onPointerUp={(event) => {
          if (event.pointerType === "mouse") return;
          window.clearTimeout(touchTimer.current);
          setActive(true);
          touchTimer.current = window.setTimeout(() => setActive(false), 2600);
        }}
        onFocus={() => setActive(true)}
        onBlur={() => setActive(false)}
      >
        <span className="home-portrait">
          <img
            src={src}
            alt=""
            aria-hidden="true"
            className={ready ? "home-portrait-source is-ready" : "home-portrait-source"}
          />
          <canvas ref={canvasRef} aria-hidden="true" className="home-portrait-canvas" />
        </span>
      </button>
    </span>
  );
}
