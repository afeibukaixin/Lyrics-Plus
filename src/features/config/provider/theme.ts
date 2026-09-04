import { useEffect, useState } from "react";

import type { ThemePreference } from "../../../shared/types";

/** 同步主题数据属性、CSS class 与系统配色监听，返回当前解析后的主题。 */
export function useResolvedTheme(theme: ThemePreference): "light" | "dark" {
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">("dark");

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved = theme === "system"
        ? (media.matches ? "dark" : "light")
        : theme;
      document.documentElement.dataset.theme = theme;
      document.documentElement.dataset.resolvedTheme = resolved;
      document.documentElement.classList.toggle("light", resolved === "light");
      document.documentElement.classList.toggle("dark", resolved === "dark");
      document.documentElement.style.colorScheme = resolved;
      setResolvedTheme(resolved);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  return resolvedTheme;
}
