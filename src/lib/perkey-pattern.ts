import type { PerkeyPattern, PerkeyValues } from "../api/lighting";

export const PERKEY_NAMES: Record<PerkeyPattern["kind"], string> = {
  static: "逐键静态", wave: "波浪", breath: "呼吸", blink: "闪烁", random: "随机", scan: "扫描",
  snake: "蛇形", ripple: "涟漪", fn: "Fn 提示", fade: "按键渐灭",
};
// BAML arrow styles and GetWaveVector/GetSnakeVector define this order, not clockwise indexing.
export const WAVE_DIRECTIONS = ["↖ 左上", "↗ 右上", "↙ 左下", "↘ 右下", "↑ 向上", "↓ 向下", "← 向左", "→ 向右"];
export const SNAKE_DIRECTIONS = ["↘ 右下", "↙ 左下", "↗ 右上", "↖ 左上"];
export function perkeySummary(values: PerkeyValues): string {
  const p = values.pattern;
  const parts = [PERKEY_NAMES[p.kind]];
  if (p.kind === "wave" || p.kind === "snake") parts.push((p.kind === "wave" ? WAVE_DIRECTIONS : SNAKE_DIRECTIONS)[p.direction]);
  if (p.kind === "ripple") parts.push(p.reactive ? "单键响应" : "扩散");
  if ("color" in p) parts.push(p.color ? `#${[p.color.r, p.color.g, p.color.b].map(n => n.toString(16).padStart(2, "0")).join("")}` : "多彩");
  if (p.kind === "scan") parts.push(p.colors ? "自定义双色" : "多彩");
  if (p.kind !== "static" && p.kind !== "fn") parts.push(`${values.speed + 1} 档`);
  parts.push(values.enabled ? `${values.brightness} 级` : "关闭");
  return parts.join(" · ");
}
