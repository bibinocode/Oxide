import { play, setEnabled } from "cuelume";

/** 交互音效偏好；存储受限时仍保留当前标签页的选择。 */
const storageKey = "oxide-sound";
const changeEvent = "oxide-sound-change";
let memoryEnabled = true;
let engineDisableTimer: ReturnType<typeof setTimeout> | undefined;

/** 默认开启，与参考站一致；SSR 不读取浏览器存储。 */
export function soundEnabled(): boolean {
  if (typeof window === "undefined") return true;
  try {
    const value = localStorage.getItem(storageKey);
    return value === null ? memoryEnabled : value === "on";
  } catch {
    return memoryEnabled;
  }
}

/** 同步当前页面与其他标签页的音效选择。 */
export function subscribeSound(listener: () => void): () => void {
  const onStorage = (event: StorageEvent) => {
    if (event.key === storageKey || event.key === null) {
      // 其他标签页关闭音效时也取消本页尚未恢复的播放，不只更新开关外观。
      setEnabled(soundEnabled());
      listener();
    }
  };
  window.addEventListener(changeEvent, listener);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(changeEvent, listener);
    window.removeEventListener("storage", onStorage);
  };
}

export function setSoundEnabled(enabled: boolean): void {
  memoryEnabled = enabled;
  try {
    localStorage.setItem(storageKey, enabled ? "on" : "off");
  } catch {
    // 私密浏览或存储额度限制不应阻止开关生效。
  }
  window.dispatchEvent(new Event(changeEvent));
  if (engineDisableTimer !== undefined) {
    clearTimeout(engineDisableTimer);
    engineDisableTimer = undefined;
  }
  if (enabled) setEnabled(true);
  else {
    // 沿用参考站的关闭节奏，先让最后一声确认音从暂停状态恢复，再禁用引擎。
    engineDisableTimer = setTimeout(() => {
      setEnabled(false);
      engineDisableTimer = undefined;
    }, 350);
  }
}

/** Cuelume 语义配方：导航 chime、偏好 success，统一遵守音效偏好。 */
export function playUiSound(cue: "navigation" | "preference" = "navigation"): void {
  if (typeof window === "undefined" || document.hidden) return;
  try {
    const enabled = soundEnabled();
    setEnabled(enabled);
    if (enabled) play(cue === "preference" ? "success" : "chime");
  } catch {
    // 浏览器不支持音频或拒绝播放时，导航与偏好操作仍正常执行。
  }
}
