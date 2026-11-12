/**
 * 主题应用唯一入口：跟随系统浅/深，写 documentElement[data-theme]，
 * tokens.css 按 data-theme 映射语义变量。设置页手动覆盖在 Phase 4 接入，
 * 接入时仍走本入口，不另存状态。
 */
export function initializeTheme(): () => void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = () => {
    document.documentElement.dataset.theme = media.matches ? "dark" : "light";
  };
  apply();
  media.addEventListener("change", apply);
  return () => media.removeEventListener("change", apply);
}
