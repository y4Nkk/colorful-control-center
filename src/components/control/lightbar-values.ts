import type { LightBarValues, RgbColor, X170LightBarEffect, X58LightBarEffect } from "../../api/client";
import { hexOf } from "../../lib/color";
export const X170_EFFECTS: { value: X170LightBarEffect; label: string }[] = [
  { value: "static", label: "静态" }, { value: "breath", label: "呼吸" }, { value: "blink", label: "闪烁" }, { value: "cycle", label: "色彩循环" },
  { value: "meteor", label: "流星" }, { value: "wave", label: "波浪" }, { value: "ripple", label: "涟漪" }, { value: "random", label: "随机" },
];
export const X58_EFFECTS: { value: X58LightBarEffect; label: string }[] = [
  { value: "static", label: "静态" }, { value: "breath", label: "呼吸" }, { value: "wave", label: "波浪" },
  { value: "cycle", label: "渐变循环" }, { value: "scan", label: "左右扫描" }, { value: "colorWave", label: "彩色波浪" },
];
export const X58_COLORS: { value: string; label: string; rgb: RgbColor }[] = [
  { value: "1", label: "红色", rgb: { r: 255, g: 0, b: 0 } }, { value: "2", label: "黄色", rgb: { r: 255, g: 255, b: 0 } },
  { value: "3", label: "黄绿色", rgb: { r: 128, g: 255, b: 0 } }, { value: "4", label: "绿色", rgb: { r: 0, g: 255, b: 0 } },
  { value: "5", label: "青色", rgb: { r: 0, g: 255, b: 255 } }, { value: "6", label: "蓝色", rgb: { r: 0, g: 0, b: 255 } },
  { value: "7", label: "紫色", rgb: { r: 128, g: 0, b: 255 } },
];
export function lightBarSummary(value: LightBarValues | null, showPowerState = true): string {
  if (!value) return "当前保存值未知";
  const settings = value.settings;
  if (value.kind === "x170") {
    const { settings } = value;
    const state = showPowerState ? settings.enabled ? " · 开启" : " · 关闭" : "";
    return `X170${state} · ${X170_EFFECTS.find(effect => effect.value === settings.effect)?.label} · 亮度 ${settings.brightness} 级 · 速度 ${settings.speed + 1} 档${settings.color ? ` · ${hexOf(settings.color).toUpperCase()}` : ""}`;
  }
  return `X58 · ${X58_EFFECTS.find(effect => effect.value === settings.effect)?.label} · 亮度 ${settings.brightness} 级 · 速度 ${settings.speed} 档${value.settings.colorId ? ` · ${X58_COLORS.find(color => color.value === String(value.settings.colorId))?.label}` : ""}`;
}
export function lightBarDraftValid(value: LightBarValues): boolean {
  if (value.kind === "x170") return true;
  const settings = value.settings, needsColor = settings.effect === "static" || settings.effect === "scan";
  return needsColor ? settings.colorId !== null && Number.isInteger(settings.colorId) && settings.colorId >= 1 && settings.colorId <= 7 : settings.colorId === null;
}
