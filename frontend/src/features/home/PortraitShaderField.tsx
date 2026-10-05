import { useCallback, useEffect, useRef, useState } from "react";
import { ContourLines, PerlinNoise, Shader } from "shaders/react";

/** 与参考站相同的噪声等高线着色器，只在悬停且 WebGPU 可用时加载。 */
export function PortraitShaderField({
  onReady,
  onUnavailable,
}: {
  onReady: () => void;
  onUnavailable: () => void;
}) {
  const [ink, setInk] = useState("#34312e");
  const [ready, setReady] = useState(false);
  const firstFrame = useRef(0);
  const secondFrame = useRef(0);

  const handleReady = useCallback(() => {
    // 等着色器完成尺寸同步和首帧绘制，再让父级从最小尺寸揭示底纹。
    cancelAnimationFrame(firstFrame.current);
    cancelAnimationFrame(secondFrame.current);
    firstFrame.current = requestAnimationFrame(() => {
      secondFrame.current = requestAnimationFrame(() => {
        setReady(true);
        onReady();
      });
    });
  }, [onReady]);

  useEffect(
    () => () => {
      cancelAnimationFrame(firstFrame.current);
      cancelAnimationFrame(secondFrame.current);
    },
    [],
  );

  useEffect(() => {
    const update = () =>
      setInk(getComputedStyle(document.documentElement).getPropertyValue("--page-text").trim());
    update();
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (ready) return;
    const timer = window.setTimeout(onUnavailable, 2500);
    return () => window.clearTimeout(timer);
  }, [onUnavailable, ready]);

  return (
    <span className="home-portrait-shader" data-ready={ready}>
      <Shader
        className="home-portrait-shader-canvas"
        colorSpace="srgb"
        disableTelemetry
        onUnavailable={onUnavailable}
        onReady={handleReady}
      >
        <ContourLines
          levels={5}
          lineWidth={0.7}
          softness={0.1}
          gamma={0.72}
          colorMode="custom"
          lineColor={ink}
          backgroundColor="transparent"
        >
          <PerlinNoise
            colorA="#ffffff"
            colorB="#000000"
            colorSpace="oklch"
            scale={2.7}
            contrast={0.08}
            balance={-0.06}
            seed={19}
            speed={0.06}
          />
        </ContourLines>
      </Shader>
    </span>
  );
}
