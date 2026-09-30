import { useEffect, useRef, useState } from "react";

/** 保持源码、预览的宽度偏好，并提供拖拽容器引用。 */
export function useEditorLayout() {
  const workspace = useRef<HTMLDivElement>(null);
  const [previewVisible, setPreviewVisible] = useState(true);
  const [split, setSplit] = useState(50);
  const [view, setView] = useState<"edit" | "preview">("edit");

  useEffect(() => {
    try {
      const preference = JSON.parse(localStorage.getItem("oxide.editor.layout") ?? "null") as {
        preview?: boolean;
        split?: number;
      } | null;
      if (typeof preference?.preview === "boolean") setPreviewVisible(preference.preview);
      if (typeof preference?.split === "number" && Number.isFinite(preference.split))
        setSplit(Math.max(20, Math.min(80, preference.split)));
    } catch {
      /* 无效或禁用的本地存储不影响编辑。 */
    }
  }, []);

  /** 布局偏好与私密恢复草稿分开，不保存 AI 对话或图片附件。 */
  function rememberLayout(preview: boolean, width: number) {
    setPreviewVisible(preview);
    setSplit(width);
    try {
      localStorage.setItem("oxide.editor.layout", JSON.stringify({ preview, split: width }));
    } catch {
      /* 隐私模式下保持本次会话的布局即可。 */
    }
  }

  return { workspace, previewVisible, split, view, setPreviewVisible, setView, rememberLayout };
}
