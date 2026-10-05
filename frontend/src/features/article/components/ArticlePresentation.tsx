import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { PixelMark } from "../../../components/layout/PrintMarks";
import { apiRequest } from "../../../lib/api/client";
import { useArticleHtml } from "../hooks/useArticleHtml";
import { useScrollReveal } from "../hooks/useScrollReveal";
import { ImageLightbox, type ZoomImage } from "./ImageLightbox";
import { ArticleCover } from "./ArticleCover";
import { articleTransitionName } from "../articleMotion";

interface ArticlePresentationProps {
  title: string;
  summary: string | null;
  publishedAt: string | null;
  html: string;
  coverUrl?: string | null;
  bodyId?: string;
  slug?: string;
}

interface LinkPreview {
  title: string;
  description: string | null;
  domain: string;
  icon_url: string | null;
  image_url: string | null;
}

interface LinkCard {
  href: string;
  title: string;
  domain: string;
  preview: LinkPreview | null;
  x: number;
  y: number;
}

/** 公开文章与编辑器预览使用同一份文章结构、封面和排版样式。 */
export function ArticlePresentation({
  title,
  summary,
  publishedAt,
  html,
  coverUrl,
  bodyId,
  slug,
}: ArticlePresentationProps) {
  const renderedHtml = useArticleHtml(html);
  const prose = useRef<HTMLDivElement>(null);
  useScrollReveal(prose, renderedHtml);
  const cache = useRef(new Map<string, Promise<LinkPreview>>());
  const activeLink = useRef<HTMLAnchorElement | null>(null);
  const [linkCard, setLinkCard] = useState<LinkCard | null>(null);
  const [imageFailed, setImageFailed] = useState(false);
  const [zoomImage, setZoomImage] = useState<ZoomImage | null>(null);
  useEffect(() => {
    const root = prose.current;
    if (!root) return;
    for (const anchor of root.querySelectorAll<HTMLAnchorElement>("a[href]")) {
      const address = new URL(anchor.href, window.location.href);
      if (!/^https?:$/.test(address.protocol) || address.host === window.location.host) continue;
      anchor.target = "_blank";
      anchor.rel = "noopener noreferrer";
      anchor.classList.add("article-external-link");
      if (!anchor.querySelector(".article-link-icon")) {
        const icon = document.createElement("img");
        icon.className = "article-link-icon";
        icon.src = `${address.origin}/favicon.ico`;
        icon.alt = "";
        icon.width = 14;
        icon.height = 14;
        icon.onerror = () => {
          icon.hidden = true;
        };
        anchor.prepend(icon);
      }
    }
    for (const image of root.querySelectorAll<HTMLImageElement>("img:not(.article-link-icon)")) {
      image.tabIndex = 0;
      image.setAttribute("role", "button");
      image.setAttribute("aria-label", `放大图片：${image.alt || "文章配图"}`);
    }
  }, [renderedHtml]);

  function openImage(image: HTMLImageElement) {
    setZoomImage({
      src: image.currentSrc || image.src,
      alt: image.alt,
      origin: image.getBoundingClientRect(),
      width: image.naturalWidth || image.width,
      height: image.naturalHeight || image.height,
    });
  }

  function hideLinkCard() {
    activeLink.current = null;
    setLinkCard(null);
  }

  /** 立即显示卡片，网页元数据异步填入；重复悬停复用同一个请求。 */
  function showLinkCard(anchor: HTMLAnchorElement) {
    if (activeLink.current === anchor || !prose.current?.contains(anchor)) return;
    activeLink.current = anchor;
    const href = anchor.href;
    const bounds = anchor.getBoundingClientRect();
    setImageFailed(false);
    setLinkCard({
      href,
      title: anchor.textContent?.trim() || new URL(href).hostname,
      domain: new URL(href).hostname,
      preview: null,
      x: Math.max(12, Math.min(bounds.left, window.innerWidth - 272)),
      y: Math.max(
        12,
        Math.min(
          bounds.bottom + 10 + 260 <= window.innerHeight ? bounds.bottom + 10 : bounds.top - 260,
          window.innerHeight - 260,
        ),
      ),
    });
    let pending = cache.current.get(href);
    if (!pending) {
      pending = apiRequest<LinkPreview>(`/api/v1/link-preview?url=${encodeURIComponent(href)}`);
      cache.current.set(href, pending);
    }
    pending
      .then((preview) => {
        if (activeLink.current !== anchor) return;
        const icon = anchor.querySelector<HTMLImageElement>(".article-link-icon");
        if (icon && preview.icon_url) {
          icon.src = preview.icon_url;
          icon.hidden = false;
        }
        setLinkCard((current) => (current?.href === href ? { ...current, preview } : current));
      })
      .catch(() => {
        cache.current.delete(href);
      });
  }
  const text = html.replace(/<[^>]*>/g, " ").replace(/&[^;]+;/g, " ");
  const units = (text.match(/[\u3400-\u9fff]|[A-Za-z0-9]+/g) ?? []).length;
  const readingMinutes = Math.max(1, Math.ceil(units / 280));
  async function copyCode(event: React.MouseEvent<HTMLDivElement>) {
    const button = (event.target as Element).closest<HTMLButtonElement>("button[data-copy-code]");
    if (!button) return;
    const code = button.closest("figure")?.querySelector("code")?.textContent;
    if (code == null) return;
    try {
      await navigator.clipboard.writeText(code);
      button.textContent = "已复制";
    } catch {
      button.textContent = "复制失败";
    }
    window.setTimeout(() => {
      if (button.isConnected) button.textContent = "复制";
    }, 1500);
  }
  return (
    <article className={`article-reader${slug ? " article-route-enter" : ""}`} lang="zh-CN">
      {coverUrl && (
        <ArticleCover key={coverUrl} src={coverUrl} title={title} slug={slug} onOpen={openImage} />
      )}
      <header className="article-title-card">
        <div className="flex items-start justify-between gap-4">
          <h1
            style={{ viewTransitionName: slug ? articleTransitionName("title", slug) : undefined }}
          >
            {title || "未命名文章"}
          </h1>
          <PixelMark />
        </div>
        <dl className="article-spec-plate">
          <div>
            <dt>类型</dt>
            <dd>
              <i aria-hidden="true" />
              文章
            </dd>
          </div>
          <div>
            <dt>日期</dt>
            <dd>
              {publishedAt
                ? new Intl.DateTimeFormat("zh-CN", {
                    year: "numeric",
                    month: "2-digit",
                    day: "2-digit",
                  }).format(new Date(publishedAt))
                : "草稿"}
            </dd>
          </div>
          <div>
            <dt>阅读</dt>
            <dd>{readingMinutes} 分钟</dd>
          </div>
          <div>
            <dt>字数</dt>
            <dd>{units.toLocaleString("zh-CN")}</dd>
          </div>
        </dl>
      </header>
      {summary && <p className="article-summary">{summary}</p>}
      <div
        id={bodyId}
        ref={prose}
        className="prose-blog article-prose"
        onClick={(event) => {
          const image = (event.target as Element).closest<HTMLImageElement>(
            "img:not(.article-link-icon)",
          );
          if (image) openImage(image);
          else void copyCode(event);
        }}
        onKeyDown={(event) => {
          if (event.key !== "Enter" && event.key !== " ") return;
          const image = (event.target as Element).closest<HTMLImageElement>(
            "img:not(.article-link-icon)",
          );
          if (image) {
            event.preventDefault();
            openImage(image);
          }
        }}
        onPointerOver={(event) => {
          if (event.pointerType === "touch") return;
          const anchor = (event.target as Element).closest<HTMLAnchorElement>(
            "a.article-external-link",
          );
          if (anchor) showLinkCard(anchor);
        }}
        onPointerOut={(event) => {
          const anchor = (event.target as Element).closest("a.article-external-link");
          if (anchor && (!event.relatedTarget || !anchor.contains(event.relatedTarget as Node))) {
            hideLinkCard();
          }
        }}
        onPointerLeave={hideLinkCard}
        onFocus={(event) => {
          const anchor = (event.target as Element).closest<HTMLAnchorElement>(
            "a.article-external-link",
          );
          if (anchor) showLinkCard(anchor);
        }}
        onBlur={hideLinkCard}
        dangerouslySetInnerHTML={{ __html: renderedHtml }}
      />
      {linkCard &&
        createPortal(
          <aside
            className="article-link-card"
            style={{ left: linkCard.x, top: linkCard.y }}
            aria-label={`${linkCard.domain} 网页预览`}
          >
            {linkCard.preview?.image_url && !imageFailed && (
              <img
                className="article-link-card-image"
                src={linkCard.preview.image_url}
                alt=""
                onError={() => setImageFailed(true)}
              />
            )}
            <span className="article-link-card-domain">
              {linkCard.preview?.icon_url && (
                <img src={linkCard.preview.icon_url} alt="" width={16} height={16} />
              )}
              {linkCard.domain}
            </span>
            <strong>{linkCard.preview?.title || linkCard.title}</strong>
            {(!linkCard.preview?.image_url || imageFailed) && linkCard.preview?.description && (
              <span className="article-link-card-description">{linkCard.preview.description}</span>
            )}
          </aside>,
          document.body,
        )}
      {zoomImage && <ImageLightbox image={zoomImage} onClose={() => setZoomImage(null)} />}
    </article>
  );
}
