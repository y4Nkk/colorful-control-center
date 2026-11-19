import { ColorPicker } from "../ColorPicker";
import { Select } from "../Select";
import type { PerkeyPattern } from "../../api/client";
import { WAVE_DIRECTIONS, SNAKE_DIRECTIONS } from "../../lib/perkey-pattern";
export function PerkeyPatternEditor({ pattern, current, advanced, disabled, onChange, onApply }: {
  pattern: PerkeyPattern; current: PerkeyPattern; advanced: boolean; disabled: boolean;
  onChange: (value: PerkeyPattern) => void; onApply: (value: PerkeyPattern) => void;
}) {
  const p = pattern;
  if (p.kind === "static" || current.kind !== p.kind) return null;
  const customizable = advanced || !["wave", "snake", "random", "scan"].includes(p.kind);
  const custom = "color" in p ? p.color !== null : p.colors !== null;
  const scanColors = p.kind === "scan" ? p.colors : null;
  const toggle = (enabled: boolean) => {
    if (current.kind === "scan") onApply({ ...current, colors: enabled ? [{ r: 0, g: 100, b: 255 }, { r: 255, g: 80, b: 0 }] : null });
    else if (current.kind !== "fn") onApply({ ...current, color: enabled ? { r: 0, g: 100, b: 255 } : null });
  };
  return <div className="ccc-perkey-options">
    {current.kind === "ripple" && <Select label="按键响应方式" value={current.reactive ? "reactive" : "ripple"} disabled={disabled}
      options={[{ value: "ripple", label: "扩散涟漪" }, { value: "reactive", label: "单键响应" }]}
      onChange={value => onApply({ ...current, reactive: value === "reactive" })} />}
    {customizable && p.kind !== "fn" && <Select label="动态配色" value={custom ? "custom" : "rainbow"} disabled={disabled}
      options={[{ value: "rainbow", label: "多彩" }, { value: "custom", label: p.kind === "scan" ? "自定义双色" : "自定义颜色" }]}
      onChange={value => toggle(value === "custom")} />}
    {(current.kind === "wave" || current.kind === "snake") && advanced && <Select label="灯效方向" value={String(current.direction)} disabled={disabled}
      options={(current.kind === "wave" ? WAVE_DIRECTIONS : SNAKE_DIRECTIONS).map((label, i) => ({ value: String(i), label }))}
      onChange={value => onApply({ ...current, direction: Number(value) })} />}
    {"color" in p && p.color && <ColorPicker label="灯效颜色" value={p.color} disabled={disabled} onChange={color => onChange({ ...p, color })} />}
    {p.kind === "scan" && scanColors?.map((color, index) => <ColorPicker key={index} label={`扫描颜色 ${index + 1}`} value={color} disabled={disabled}
      onChange={next => onChange({ ...p, colors: index === 0 ? [next, scanColors[1]] : [scanColors[0], next] })} />)}
    {custom && <span className="ccc-control-caption">{p.kind === "scan" ? "双色不能同时为黑色。" : "动态自定义颜色需为非黑色。"}</span>}
  </div>;
}
