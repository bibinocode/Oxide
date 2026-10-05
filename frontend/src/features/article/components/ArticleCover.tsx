import { useEffect, useRef, useState } from "react";
import { articleTransitionName } from "../articleMotion";

/** 相纸框保持固定比例；原图加载后显影，路由过渡则由同名照片区域承担。 */
export function ArticleCover({
  src,
  title,
  slug,
  onOpen,
}: {
  src: string;
  title: string;
  slug?: string;
  onOpen?: (image: HTMLImageElement) => void;
}) {
  const image = useRef<HTMLImageElement>(null);
  const [phase, setPhase] = useState<"loading" | "develop" | "morph">("loading");
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    // 缓存命中或 SSR 提前加载的图片也进入显影状态，不依赖再次触发 load。
    if (image.current?.complete && image.current.naturalWidth > 0) markLoaded();
  }, []);

  function markLoaded() {
    // 共享过渡已经承担图片入场，避免把透明动画首帧捕获进照片快照后再次闪烁。
    const morphing =
      CSS.supports("selector(:active-view-transition-type(article))") &&
      document.documentElement.matches(":active-view-transition-type(article)");
    setPhase(morphing ? "morph" : "develop");
  }

  return (
    <figure className="article-cover">
      <div
        className="article-cover-photo"
        style={{ viewTransitionName: slug ? articleTransitionName("cover", slug) : undefined }}
      >
        {failed ? (
          <span className="article-cover-unavailable">主图暂时无法加载</span>
        ) : (
          <img
            ref={image}
            src={src}
            alt={title ? `${title} · 主图` : "文章主图"}
            className={
              phase === "develop" ? "article-cover-image is-developed" : "article-cover-image"
            }
            fetchPriority="high"
            decoding="async"
            role={onOpen ? "button" : undefined}
            tabIndex={onOpen ? 0 : undefined}
            aria-label={onOpen ? "放大文章主图" : undefined}
            onLoad={markLoaded}
            onError={() => setFailed(true)}
            onClick={onOpen ? (event) => onOpen(event.currentTarget) : undefined}
            onKeyDown={
              onOpen
                ? (event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onOpen(event.currentTarget);
                    }
                  }
                : undefined
            }
          />
        )}
      </div>
      <figcaption>{title || "文章主图"}</figcaption>
    </figure>
  );
}
