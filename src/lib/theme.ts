import { ipc } from "@/lib/ipc";

export type Theme = "light" | "dark";

const THEME_KEY = "ui.theme";

export function applyTheme(theme: Theme): void {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function systemTheme(): Theme {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Load the persisted theme (falling back to the OS preference) and apply it. */
export async function initTheme(): Promise<Theme> {
  let theme: Theme = systemTheme();
  try {
    const saved = await ipc.getMeta(THEME_KEY);
    if (saved === "light" || saved === "dark") theme = saved;
  } catch {
    // Running outside Tauri (e.g. tests) — fall back to system.
  }
  applyTheme(theme);
  return theme;
}

/** Apply + persist a theme choice. */
export async function setTheme(theme: Theme): Promise<void> {
  applyTheme(theme);
  try {
    await ipc.setMeta(THEME_KEY, theme);
  } catch {
    // Persistence is best-effort.
  }
}
