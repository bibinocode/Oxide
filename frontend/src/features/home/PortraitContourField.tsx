import { Suspense, lazy, useEffect, useRef, useState } from "react";

const PortraitShaderField = lazy(() =>
  import("./PortraitShaderField").then((module) => ({ default: module.PortraitShaderField })),
);

const CONTOUR_LEVELS = [-0.3, -0.16, -0.02, 0.12, 0.26];
const EDGE_PAIRS: ReadonlyArray<ReadonlyArray<readonly [number, number]>> = [
  [],
  [[3, 0]],
  [[0, 1]],
  [[3, 1]],
  [[1, 2]],
  [
    [3, 0],
    [1, 2],
  ],
  [[0, 2]],
  [[3, 2]],
  [[2, 3]],
  [[0, 2]],
  [
    [0, 1],
    [2, 3],
  ],
  [[1, 2]],
  [[1, 3]],
  [[0, 1]],
  [[3, 0]],
  [],
];

function smooth(value: number) {
  return value * value * (3 - 2 * value);
}

function random(x: number, y: number) {
  const value = Math.sin(x * 127.1 + y * 311.7 + 19) * 43758.5453;
  return (value - Math.floor(value)) * 2 - 1;
}

/** 相邻格点插值为连续的二维噪声，供等值线提取。 */
function noise(x: number, y: number) {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = smooth(x - ix);
  const fy = smooth(y - iy);
  const top = random(ix, iy) * (1 - fx) + random(ix + 1, iy) * fx;
  const bottom = random(ix, iy + 1) * (1 - fx) + random(ix + 1, iy + 1) * fx;
  return top * (1 - fy) + bottom * fy;
}

function edgePoint(
  edge: number,
  values: readonly number[],
  level: number,
  x: number,
  y: number,
  step: number,
) {
  const corners: readonly [number, number][] = [
    [0, 0],
    [1, 0],
    [1, 1],
    [0, 1],
  ];
  const start = edge;
  const end = (edge + 1) % 4;
  const amount = Math.max(0, Math.min(1, (level - values[start]) / (values[end] - values[start])));
  return [
    x + (corners[start][0] + (corners[end][0] - corners[start][0]) * amount) * step,
    y + (corners[start][1] + (corners[end][1] - corners[start][1]) * amount) * step,
  ] as const;
}

/** 肖像背后的低对比度等高线，只在揭示状态绘制和运行。 */
function CanvasContourField({ active, motion }: { active: boolean; motion: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !active) return;
    let frame = 0;
    const started = performance.now();
    let lastDraw = 0;

    function draw(now: number) {
      if (!canvas || !context) return;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      if (!width || !height) return;
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      if (
        canvas.width !== Math.round(width * ratio) ||
        canvas.height !== Math.round(height * ratio)
      ) {
        canvas.width = Math.round(width * ratio);
        canvas.height = Math.round(height * ratio);
      }
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height);
      context.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue(
        "--page-text",
      );
      context.lineWidth = 0.75;
      context.globalAlpha = 0.24;
      const step = 5;
      const columns = Math.ceil(width / step);
      const rows = Math.ceil(height / step);
      const stride = columns + 1;
      const values = new Float32Array(stride * (rows + 1));
      const time = motion ? (now - started) * 0.00003 : 0;
      for (let y = 0; y <= rows; y++) {
        for (let x = 0; x <= columns; x++) {
          const nx = ((x * step) / width) * 5.8 + time;
          const ny = ((y * step) / height) * 5.8 - time * 0.7;
          const warp = noise(nx * 0.6 + 3.2, ny * 0.6 - 1.7) * 0.48;
          values[y * stride + x] =
            noise(nx + warp, ny - warp * 0.7) * 0.72 + noise(nx * 2.1 - 2.4, ny * 2.1 + 4.6) * 0.28;
        }
      }
      for (const level of CONTOUR_LEVELS) {
        context.beginPath();
        for (let y = 0; y < rows; y++) {
          for (let x = 0; x < columns; x++) {
            const topLeft = y * stride + x;
            const corners = [
              values[topLeft],
              values[topLeft + 1],
              values[topLeft + stride + 1],
              values[topLeft + stride],
            ];
            const mask = corners.reduce(
              (result, value, index) => result | (value >= level ? 1 << index : 0),
              0,
            );
            for (const [first, second] of EDGE_PAIRS[mask]) {
              const a = edgePoint(first, corners, level, x * step, y * step, step);
              const b = edgePoint(second, corners, level, x * step, y * step, step);
              context.moveTo(a[0], a[1]);
              context.lineTo(b[0], b[1]);
            }
          }
        }
        context.stroke();
      }
      context.globalAlpha = 1;
    }

    function tick(now: number) {
      if (now - lastDraw >= 40) {
        lastDraw = now;
        draw(now);
      }
      frame = requestAnimationFrame(tick);
    }

    const resize = new ResizeObserver(() => draw(performance.now()));
    resize.observe(canvas);
    draw(performance.now());
    if (motion) frame = requestAnimationFrame(tick);
    return () => {
      resize.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [active, motion]);

  return <canvas ref={canvasRef} aria-hidden="true" className="home-portrait-contours" />;
}

/** WebGPU 优先使用参考站的同款着色器，其他设备保留静态或低帧率 Canvas 底纹。 */
export function PortraitContourField({ active, motion }: { active: boolean; motion: boolean }) {
  const [supported, setSupported] = useState<boolean | null>(null);
  const [shaderReady, setShaderReady] = useState(false);

  useEffect(() => {
    if (!active || !motion || supported !== null) return;
    let cancelled = false;
    const gpu = (
      navigator as Navigator & { gpu?: { requestAdapter: () => Promise<unknown | null> } }
    ).gpu;
    if (!gpu) {
      setSupported(false);
      return;
    }
    void gpu
      .requestAdapter()
      .then((adapter) => {
        if (!cancelled) setSupported(Boolean(adapter));
      })
      .catch(() => {
        if (!cancelled) setSupported(false);
      });
    return () => {
      cancelled = true;
    };
  }, [active, motion, supported]);

  useEffect(() => {
    if (!active) setShaderReady(false);
  }, [active]);

  const showShader = active && motion && supported === true;
  return (
    <span className="home-portrait-field" data-active={active} aria-hidden="true">
      {(!showShader || !shaderReady) && (
        <CanvasContourField active={active} motion={motion && !showShader} />
      )}
      {showShader && (
        <Suspense fallback={null}>
          <PortraitShaderField
            onReady={() => setShaderReady(true)}
            onUnavailable={() => {
              setShaderReady(false);
              setSupported(false);
            }}
          />
        </Suspense>
      )}
    </span>
  );
}
