import { useCallback, useEffect, useRef, useState } from "react";

const DITHER_MATRIX = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

/** 低分辨率有序抖动保留纸面印刷质感，悬停文章行时显露原图。 */
export function ArticlePrint({
  src,
  transitionName,
}: {
  src?: string | null;
  transitionName?: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const source = useRef<HTMLImageElement>(null);
  const [ready, setReady] = useState(false);

  const print = useCallback((image: HTMLImageElement) => {
    const context = canvas.current?.getContext("2d", { willReadFrequently: true });
    if (!context) return;
    try {
      const width = 128;
      const height = 88;
      const scale = Math.max(width / image.naturalWidth, height / image.naturalHeight);
      const croppedWidth = image.naturalWidth * scale;
      const croppedHeight = image.naturalHeight * scale;
      context.drawImage(
        image,
        (width - croppedWidth) / 2,
        (height - croppedHeight) / 2,
        croppedWidth,
        croppedHeight,
      );
      const pixels = context.getImageData(0, 0, width, height);
      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          const offset = (y * width + x) * 4;
          const luminance =
            (pixels.data[offset]! * 0.2126 +
              pixels.data[offset + 1]! * 0.7152 +
              pixels.data[offset + 2]! * 0.0722) /
            255;
          const threshold = (DITHER_MATRIX[(y % 4) * 4 + (x % 4)]! + 0.5) / 16;
          const ink = luminance < threshold ? 38 : 246;
          pixels.data[offset] = pixels.data[offset + 1] = pixels.data[offset + 2] = ink;
        }
      }
      context.putImageData(pixels, 0, 0);
      setReady(true);
    } catch {
      // 外部图片未允许跨域读取时保留原图，避免破坏列表内容。
      setReady(false);
    }
  }, []);

  useEffect(() => {
    // SSR 图片可能在 hydration 前已加载，补绘缓存图片，避免首屏漏掉印刷层。
    if (source.current?.complete && source.current.naturalWidth > 0) print(source.current);
  }, [src, print]);

  return (
    <span className="article-print-pile" aria-hidden="true">
      <span className="article-print-sheet" />
      <span className="article-print-sheet" />
      <span
        className="article-print-thumb"
        style={{ viewTransitionName: src ? transitionName : undefined }}
      >
        {src && (
          <img
            ref={source}
            key={src}
            src={src}
            alt=""
            loading="lazy"
            width={64}
            height={44}
            onLoad={(event) => print(event.currentTarget)}
          />
        )}
        {src && (
          <canvas
            ref={canvas}
            width={128}
            height={88}
            className={ready ? "article-print-dither is-ready" : "article-print-dither"}
          />
        )}
      </span>
    </span>
  );
}
