import { useCallback, useEffect, useRef, useState } from "react";
import { FlowField, Grid, Shader } from "shaders/react";

/** 参考站同参数的微流动网格；仅在项目交互期间挂载。 */
export function ProjectShaderField({
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
    // 等着色器完成尺寸同步和首帧绘制，再让父级显示动态网格。
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
    <span className="project-shader" data-ready={ready}>
      <Shader
        className="project-shader-canvas"
        colorSpace="srgb"
        disableTelemetry
        onUnavailable={onUnavailable}
        onReady={handleReady}
      >
        <FlowField
          strength={0.035}
          detail={1.6}
          speed={0.1}
          evolutionSpeed={0.06}
          seed={31}
          edges="wrap"
        >
          <Grid
            color={ink}
            cellColor="transparent"
            cells={22}
            thickness={0.6}
            softness={0.08}
            variation={0}
            rotation={0}
            colorSpace="oklab"
          />
        </FlowField>
      </Shader>
    </span>
  );
}
