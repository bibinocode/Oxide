import { useEffect, useId, useRef, useState } from "react";
import { useRouterState } from "@tanstack/react-router";
import { Volume2, VolumeX, SlidersHorizontal } from "lucide-react";
import { DockTooltip } from "./DockTooltip";
import { useSoundPreference } from "../../hooks/useSoundPreference";
import { playUiSound } from "../../lib/sound";
import { ThemeControl } from "./ThemeControl";

/** 固定导航的偏好面板；原生 Popover 处理点击外部、Escape 和顶层展示。 */
export function SitePreferences() {
  const id = useId();
  const { enabled, setEnabled } = useSoundPreference();
  const panel = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const pathname = useRouterState({ select: (state) => state.location.pathname });

  useEffect(() => {
    panel.current?.hidePopover();
  }, [pathname]);

  useEffect(() => {
    if (!open) return;
    // 打开时定位当前选择；Tab 离开后关闭，不把焦点困在非模态面板中。
    panel.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
    const onFocus = (event: FocusEvent) => {
      if (
        event.target instanceof Node &&
        !panel.current?.contains(event.target) &&
        !trigger.current?.contains(event.target)
      ) {
        panel.current?.hidePopover();
      }
    };
    document.addEventListener("focusin", onFocus);
    return () => document.removeEventListener("focusin", onFocus);
  }, [open]);

  return (
    <div className="site-preferences">
      <button
        ref={trigger}
        type="button"
        className="preferences-trigger"
        onClick={() => playUiSound()}
        popoverTarget={id}
        aria-label="偏好设置"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={id}
      >
        <SlidersHorizontal size={18} aria-hidden="true" />
        <DockTooltip label="偏好" />
      </button>
      <div
        ref={panel}
        id={id}
        popover="auto"
        role="dialog"
        aria-label="偏好设置"
        className="preferences-panel"
        onToggle={(event) => setOpen(event.newState === "open")}
      >
        <div className="preferences-row">
          <span className="preferences-row-label">外观</span>
          <ThemeControl variant="preferences" />
        </div>
        <div className="preferences-row">
          <span className="preferences-row-label">音效</span>
          <div className="theme-control theme-control-preferences" role="group" aria-label="音效">
            {[
              { value: true, label: "开启音效", Icon: Volume2 },
              { value: false, label: "关闭音效", Icon: VolumeX },
            ].map(({ value, label, Icon }) => (
              <button
                key={label}
                type="button"
                aria-label={label}
                aria-pressed={enabled === value}
                onClick={() => {
                  if (!value && enabled) playUiSound("preference");
                  setEnabled(value);
                  if (value) playUiSound("preference");
                }}
              >
                <Icon size={21} aria-hidden="true" />
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
