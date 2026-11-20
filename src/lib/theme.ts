import { useEffect, useState } from "react";

/** "system" follows Windows; the others force a theme. */
export type Theme = "system" | "light" | "dark";

const KEY = "melomango.theme";

export function savedTheme(): Theme {
  try {
    const value = localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

/** Sets `data-theme` on <html> (none for "system", so the OS preference applies). */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/** The chosen theme, applied and remembered. */
export function useTheme(): [Theme, (theme: Theme) => void] {
  const [theme, setTheme] = useState<Theme>(savedTheme);
  useEffect(() => {
    applyTheme(theme);
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      /* storage unavailable: not important */
    }
  }, [theme]);
  return [theme, setTheme];
}
