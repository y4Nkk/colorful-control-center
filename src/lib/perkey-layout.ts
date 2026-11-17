import facts from "./perkey-layout-data.json";
import type { PerkeyDevice } from "../api/lighting";

export interface Keycap { id: string; label: string; legend: string; leds: number[]; x: number; y: number; width: number; height: number }
const symbols: Record<string, string> = {
  "20": "`", "2b": "−", "2d": "=", "4c": "[", "4d": "]", "6b": ";", "6c": "'",
  "8a": ",", "8b": ".", "8c": "/", "4041": "Tab", "4e4f": "\\", "6e6f": "Enter", "6f": "Enter",
  "8082": "Shift", "80": "Shift", "82": "\\", "8d8e": "Shift", "8d": "\\", "8e": "Shift",
  "a3": "Win", "ab": "菜单", "a5a9": "空格", "a6a8": "空格", "2e2f": "Backspace",
  "8f": "↑", "ae": "←", "af": "↓", "b0": "→",
};
const legends: Record<string, string> = {
  Backspace: "⌫", "Caps Lock": "Caps", "Num LK": "Num", Win: "⊞", 菜单: "≡", "Prt Sc SysRq": "Prt",
  "Ins Scr LK": "Ins", "PgUP Pause": "Pg↑", "PgDn Break": "Pg↓", PgUP: "Pg↑", PgDn: "Pg↓", Home: "Hm",
};
export function ledIndices(code: string, narrow: boolean): number[] {
  const value = Number.parseInt(code, 16);
  const ids = [value & 255, ...(value > 255 ? [value >> 8] : [])];
  if (code === "a5a9" && narrow) ids.push(0xA6, 0xA8);
  if (code === "6f") ids.push(0x4E, 0x4F);
  return [...new Set(ids)].map(id => (id >> 5) * 20 + (id & 31));
}
export function perkeyLayout(device: PerkeyDevice): { keys: Keycap[]; width: number; height: number } | null {
  if (device.language > 3 || device.compact === null || device.physicalThreeColumn === null) return null;
  const name = `${device.language}-${Number(device.physicalThreeColumn)}-${Number(device.compact)}` as keyof typeof facts.layouts;
  const indexes = facts.layouts[name];
  if (!indexes) return null;
  const keys = indexes.map(index => {
    const key = facts.keys[index];
    const raw = key.label?.trim().replace(/\s+/g, " ");
    const label = raw && !["US", "UK", "JB", "BR", "JP3C"].includes(raw) && !raw.includes("�")
      ? raw.replace("Crtl", "Ctrl") : symbols[key.code] ?? key.code.toUpperCase();
    // The source function row is too short at dashboard scale; keep its LED map and give labels room.
    return { ...key, y: key.y < 45 ? key.y : key.y + 24, height: key.y < 45 ? key.height + 24 : key.height,
      label, legend: legends[label] ?? (key.code === "93b3" ? "↵" : label), leds: ledIndices(key.code, device.narrowBorder) };
  });
  return { keys, width: Math.max(...keys.map(k => k.x + k.width)), height: Math.max(...keys.map(k => k.y + k.height)) };
}
