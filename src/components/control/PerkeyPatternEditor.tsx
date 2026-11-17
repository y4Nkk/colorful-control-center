import { Input } from "../Input";
import { Select } from "../Select";
import { hexOf, rgbOf } from "./useKeyboardLighting";
import type { PerkeyPattern } from "../../api/client";
import { WAVE_DIRECTIONS, SNAKE_DIRECTIONS } from "../../lib/perkey-pattern";
export function PerkeyPatternEditor({ pattern, advanced, disabled, onChange }: {
  pattern: PerkeyPattern; advanced: boolean; disabled: boolean; onChange: (value: PerkeyPattern) => void;
}) {
  const p = pattern;
  if (p.kind === "static") return null;
  const customizable = advanced || !["wave", "snake", "random", "scan"].includes(p.kind);
  const custom = "color" in p ? p.color !== null : p.colors !== null;
  const toggle = (enabled: boolean) => {
    if (p.kind === "scan") onChange({ ...p, colors: enabled ? [{ r: 0, g: 100, b: 255 }, { r: 255, g: 80, b: 0 }] : null });
    else if (p.kind !== "fn") onChange({ ...p, color: enabled ? { r: 0, g: 100, b: 255 } : null });
  };
  return <div className="ccc-perkey-options">
    {p.kind === "ripple" && <Select label="按键响应方式" value={p.reactive ? "reactive" : "ripple"} disabled={disabled}
      options={[{ value: "ripple", label: "扩散涟漪" }, { value: "reactive", label: "单键响应" }]}
      onChange={value => onChange({ ...p, reactive: value === "reactive" })} />}
    {customizable && p.kind !== "fn" && <Select label="动态配色" value={custom ? "custom" : "rainbow"} disabled={disabled}
      options={[{ value: "rainbow", label: "多彩" }, { value: "custom", label: p.kind === "scan" ? "自定义双色" : "自定义颜色" }]}
      onChange={value => toggle(value === "custom")} />}
    {(p.kind === "wave" || p.kind === "snake") && advanced && <Select label="灯效方向" value={String(p.direction)} disabled={disabled}
      options={(p.kind === "wave" ? WAVE_DIRECTIONS : SNAKE_DIRECTIONS).map((label, i) => ({ value: String(i), label }))}
      onChange={value => onChange({ ...p, direction: Number(value) })} />}
    {"color" in p && p.color && <label className="ccc-color-picker">灯效颜色<Input type="color" aria-label="灯效颜色" value={hexOf(p.color)} disabled={disabled}
      onChange={e => onChange({ ...p, color: rgbOf(e.target.value) })} /></label>}
    {p.kind === "scan" && p.colors && p.colors.map((color, index) => <label className="ccc-color-picker" key={index}>颜色 {index + 1}
      <Input type="color" aria-label={`扫描颜色 ${index + 1}`} value={hexOf(color)} disabled={disabled} onChange={e => {
        const colors = [...p.colors!] as NonNullable<typeof p.colors>; colors[index] = rgbOf(e.target.value); onChange({ ...p, colors });
      }} /></label>)}
    {custom && <span className="ccc-control-caption">{p.kind === "scan" ? "双色不能同时为黑色。" : "动态自定义颜色需为非黑色。"}</span>}
  </div>;
}
