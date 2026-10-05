import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ImagePlus, Sparkles, X } from "lucide-react";
import { apiRequest } from "../../../lib/api/client";
import { csrfHeaders, useAdminSession } from "../../admin/AdminSession";

export interface GeneratedImage {
  asset_public_id: string;
  media_url: string;
}

/** 私有生成素材经管理员确认后才公开并进入主图或正文。 */
export function ImageGenerator({
  purpose,
  title,
  source,
  onUse,
  onClose,
}: {
  purpose: "cover" | "inline";
  title: string;
  source: string;
  onUse: (image: GeneratedImage, description: string) => void;
  onClose: () => void;
}) {
  const { session } = useAdminSession();
  const dialog = useRef<HTMLDialogElement>(null);
  const [mounted, setMounted] = useState(false);
  const [instruction, setInstruction] = useState("");
  const [image, setImage] = useState<GeneratedImage | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    setMounted(true);
    return () => setMounted(false);
  }, []);
  useEffect(() => {
    if (mounted) dialog.current?.showModal();
  }, [mounted]);

  async function generate() {
    if (!session) return;
    setPending(true);
    setImage(null);
    setError("");
    try {
      setImage(
        await apiRequest<GeneratedImage>("/api/v1/admin/agent/image", {
          method: "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({ title, source, instruction, purpose }),
        }),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "生成图片失败");
    } finally {
      setPending(false);
    }
  }

  async function useImage() {
    if (!session || !image) return;
    setPending(true);
    setError("");
    try {
      await apiRequest(`/api/v1/admin/assets/${image.asset_public_id}`, {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ public: true }),
      });
      onUse(image, instruction.trim() || title.trim());
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "采用图片失败");
    } finally {
      setPending(false);
    }
  }

  if (!mounted) return null;
  return createPortal(
    <dialog
      ref={dialog}
      className="image-generator-dialog"
      onCancel={onClose}
      aria-label="生成文章图片"
    >
      <header className="flex items-center justify-between gap-4">
        <div>
          <p className="eyebrow">IMAGE</p>
          <h2 className="mt-2 text-base font-semibold">
            {purpose === "cover" ? "生成文章主图" : "生成正文配图"}
          </h2>
        </div>
        <button type="button" aria-label="关闭生图" onClick={onClose}>
          <X size={18} />
        </button>
      </header>
      <label className="mt-6 block text-sm">
        画面要求
        <textarea
          className="field mt-2 min-h-24"
          maxLength={500}
          value={instruction}
          placeholder="例如：具体场景、主体、构图或配色"
          onChange={(event) => setInstruction(event.target.value)}
        />
      </label>
      {image && (
        <figure className="image-generator-preview mt-5">
          <img src={image.media_url} alt="生成的图片预览" />
        </figure>
      )}
      {error && (
        <p className="mt-4 text-sm text-warm" role="alert">
          {error}
        </p>
      )}
      <div className="mt-5 flex flex-wrap justify-end gap-2">
        <button
          type="button"
          className="button-secondary"
          disabled={pending}
          onClick={() => void generate()}
        >
          <Sparkles size={16} />
          {pending && !image ? "生成中…" : image ? "重新生成" : "生成图片"}
        </button>
        {image && (
          <button
            type="button"
            className="button-primary"
            disabled={pending}
            onClick={() => void useImage()}
          >
            <ImagePlus size={16} />
            {pending ? "处理中…" : purpose === "cover" ? "设为主图" : "插入正文"}
          </button>
        )}
      </div>
    </dialog>,
    document.body,
  );
}
