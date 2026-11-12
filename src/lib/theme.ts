/**
 * 主题应用唯一入口：默认跟随系统浅/深，可被设置页覆盖（localStorage
 * `ccc-theme`），写 documentElement[data-theme]，tokens.css 按 data-theme
 * 映射语义变量。所有主题变更必须走本入口，不另存状态。
 */

export type ThemePreference = "system" | "light" | "dark";

const STORAGE_KEY = "ccc-theme";

export function getThemePreference(): ThemePreference {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" || stored === "dark" ? stored : "system";
}

export function setThemePreference(preference: ThemePreference): void {
  localStorage.setItem(STORAGE_KEY, preference);
  apply(preference);
}

function resolve(preference: ThemePreference): "light" | "dark" {
  if (preference !== "system") return preference;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function apply(preference: ThemePreference): void {
  document.documentElement.dataset.theme = resolve(preference);
}

/** app 根调用一次：应用当前偏好并跟随系统变化。返回清理函数。 */
export function initializeTheme(): () => void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const preference = getThemePreference();
  apply(preference);
  // 手动覆盖时不跟随系统；跟随系统偏好时才挂监听
  const onChange = () => {
    if (getThemePreference() === "system") apply("system");
  };
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}
