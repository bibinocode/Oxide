import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

export interface ZoomImage {
  src: string;
  alt: string;
  origin: DOMRect;
  width: number;
  height: number;
}

/** 从正文图片的位置平滑放大，并在关闭时返回原位置。 */
export function ImageLightbox({ image, onClose }: { image: ZoomImage; onClose: () => void }) {
  const [expanded, setExpanded] = useState(false);
  const [closing, setClosing] = useState(false);
  const closeButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const frame = requestAnimationFrame(() => setExpanded(true));
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButton.current?.focus();
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setClosing(true);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      document.body.style.overflow = previousOverflow;
      previousFocus?.focus();
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);
  useEffect(() => {
    if (!closing) return;
    const frame = requestAnimationFrame(() => setExpanded(false));
    const timer = window.setTimeout(onClose, 280);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [closing, onClose]);

  const availableWidth = Math.max(1, window.innerWidth - 48);
  const availableHeight = Math.max(1, window.innerHeight - 80);
  const scale = Math.min(availableWidth / image.width, availableHeight / image.height, 1);
  const width = image.width * scale;
  const height = image.height * scale;
  const target = {
    left: (window.innerWidth - width) / 2,
    top: (window.innerHeight - height) / 2,
    width,
    height,
  };
  const source = {
    left: image.origin.left,
    top: image.origin.top,
    width: image.origin.width,
    height: image.origin.height,
  };

  return createPortal(
    <div
      className={`article-lightbox ${expanded ? "is-open" : ""}`}
      role="dialog"
      aria-modal="true"
      aria-label={image.alt || "查看图片"}
      onClick={() => setClosing(true)}
    >
      <button
        ref={closeButton}
        type="button"
        className="article-lightbox-close"
        aria-label="关闭图片"
        title="关闭图片"
        onClick={() => setClosing(true)}
      >
        <X size={20} />
      </button>
      <img
        src={image.src}
        alt={image.alt}
        className="article-lightbox-image"
        style={expanded ? target : source}
        onClick={(event) => event.stopPropagation()}
      />
    </div>,
    document.body,
  );
}
