import { isDesktopWindow, type Appearance } from "../api/client";

let current: Appearance | null = null;

export function interfaceFontStack(font: string) {
  return font === "system" ? "var(--ccc-font-default)"
    : `${JSON.stringify(font)}, var(--ccc-font-default)`;
}

function applyTheme() {
  const theme = current?.theme ?? "system";
  document.documentElement.dataset.theme = theme === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light" : theme;
}

export function applyAppearance(settings: Appearance) {
  current = settings;
  applyTheme();
  const root = document.documentElement;
  root.dataset.motion = settings.motion;
  root.style.setProperty("--ccc-font-sans", interfaceFontStack(settings.interfaceFont));
  if (!isDesktopWindow()) root.style.zoom = String(settings.interfaceScale / 100);
}

export function observeSystemTheme() {
  applyTheme();
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", applyTheme);
  return () => media.removeEventListener("change", applyTheme);
}
