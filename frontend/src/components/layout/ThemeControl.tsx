import { useEffect, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

type Theme = "system" | "light" | "dark";
const storageKey = "oxide-theme";
const options = [
  { value: "system", label: "跟随系统", Icon: Monitor },
  { value: "light", label: "浅色", Icon: Sun },
  { value: "dark", label: "深色", Icon: Moon },
] as const;

/** 三态主题设置写入本地存储，系统模式跟随系统颜色变化。 */
function storedTheme(): Theme {
  try {
    const value = localStorage.getItem(storageKey);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

function applyTheme() {
  const theme = storedTheme();
  document.documentElement.dataset.theme =
    theme === "system"
      ? matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
}

export function ThemeControl({ compact = false }: { compact?: boolean }) {
  const [theme, setTheme] = useState<Theme>("system");
  useEffect(() => {
    const sync = () => {
      setTheme(storedTheme());
      applyTheme();
    };
    sync();
    const media = matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", sync);
    window.addEventListener("storage", sync);
    window.addEventListener("oxide-theme-change", sync);
    return () => {
      media.removeEventListener("change", sync);
      window.removeEventListener("storage", sync);
      window.removeEventListener("oxide-theme-change", sync);
    };
  }, []);

  function select(next: Theme) {
    try {
      localStorage.setItem(storageKey, next);
    } catch {
      // 私密浏览模式可能禁用存储，此时选择仅在当前页面生效。
      document.documentElement.dataset.theme =
        next === "system"
          ? matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light"
          : next;
      setTheme(next);
      return;
    }
    setTheme(next);
    window.dispatchEvent(new Event("oxide-theme-change"));
  }

  return (
    <div
      className={`theme-control ${compact ? "theme-control-compact" : ""}`}
      role="group"
      aria-label="外观主题"
    >
      {options.map(({ value, label, Icon }) => (
        <button
          key={value}
          type="button"
          aria-label={label}
          title={label}
          aria-pressed={theme === value}
          onClick={() => select(value)}
        >
          <Icon size={15} />
        </button>
      ))}
    </div>
  );
}
