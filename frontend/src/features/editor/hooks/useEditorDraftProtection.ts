import { useEffect, useRef, useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { storeEditorDraft, type StoredEditorDraft } from "../editorDraft";

/** 高频编辑先写恢复草稿；pagehide 同步补写，不依赖网络请求赶在刷新前完成。 */
export function useEditorDraftProtection({
  storageKey,
  ready,
  draft,
  dirty,
}: {
  storageKey: string | null;
  ready: boolean;
  draft: StoredEditorDraft;
  dirty: boolean;
}) {
  const current = useRef({ storageKey, ready, draft, dirty });
  const discarded = useRef(false);
  current.current = {
    storageKey,
    ready: ready && !discarded.current,
    draft,
    dirty: dirty && !discarded.current,
  };
  const [error, setError] = useState("");
  const fingerprint = JSON.stringify(draft.values);
  const [protectedValue, setProtectedValue] = useState("");
  function flush() {
    const state = current.current;
    if (!state.ready || !state.storageKey) return;
    try {
      if (state.dirty || (state.storageKey.endsWith(":new") && state.draft.savedId))
        storeEditorDraft(state.storageKey, { ...state.draft, savedAt: new Date().toISOString() });
      else localStorage.removeItem(state.storageKey);
      setProtectedValue(JSON.stringify(state.draft.values));
      setError("");
      return true;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "本地保存失败，请勿直接关闭页面");
      return false;
    }
  }
  useEffect(() => {
    if (!ready) return;
    const timer = window.setTimeout(flush, 200);
    return () => window.clearTimeout(timer);
  }, [storageKey, ready, fingerprint, dirty, draft.savedId, draft.serverUpdatedAt]);
  useEffect(() => {
    const hide = () => flush();
    const visibility = () => {
      if (document.visibilityState === "hidden") flush();
    };
    window.addEventListener("pagehide", hide);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      flush();
      window.removeEventListener("pagehide", hide);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, []);
  useBlocker({
    shouldBlockFn: () => {
      const saved = flush();
      return !!current.current.dirty && !saved && !window.confirm("草稿尚未安全保存，确定离开？");
    },
    enableBeforeUnload: ready && dirty && (!!error || protectedValue !== fingerprint),
  });
  function discard() {
    discarded.current = true;
    const key = current.current.storageKey;
    current.current = { ...current.current, ready: false, dirty: false };
    if (key) localStorage.removeItem(key);
  }
  return { error, protected: protectedValue === fingerprint && !error, flush, discard };
}
