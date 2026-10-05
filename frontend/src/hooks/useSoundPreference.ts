import { useSyncExternalStore } from "react";
import { setSoundEnabled, soundEnabled, subscribeSound } from "../lib/sound";

const serverSnapshot = () => true;

/** 使用一致的 SSR 快照，并订阅本地与跨标签页偏好变更。 */
export function useSoundPreference(): {
  enabled: boolean;
  setEnabled: (enabled: boolean) => void;
} {
  const enabled = useSyncExternalStore(subscribeSound, soundEnabled, serverSnapshot);
  return { enabled, setEnabled: setSoundEnabled };
}
