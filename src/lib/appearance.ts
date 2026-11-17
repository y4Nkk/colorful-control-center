import { isDesktopWindow, type Appearance } from "../api/client";

let current: Appearance | null = null;
function applyTheme() {
  if (!current) return;
  document.documentElement.dataset.theme = current.theme === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light" : current.theme;
}

export function applyAppearance(settings: Appearance) {
  current = settings;
  applyTheme();
  const root = document.documentElement;
  root.dataset.motion = settings.motion;
  root.style.fontFamily = settings.interfaceFont === "system" ? "var(--ccc-font-sans)"
    : `${JSON.stringify(settings.interfaceFont)}, var(--ccc-font-sans)`;
  if (!isDesktopWindow()) root.style.zoom = String(settings.interfaceScale / 100);
}

export function observeSystemTheme() {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", applyTheme);
  return () => media.removeEventListener("change", applyTheme);
}
