import type { LightBarValues, X170LightBarSettings, X58LightBarSettings } from "../../api/client";
import { Checkbox } from "../Checkbox";
import { Input } from "../Input";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ScaleSlider } from "./ScaleSlider";
import { X170_EFFECTS, X58_EFFECTS, X58_COLORS } from "./lightbar-values";
import { hexOf, rgbOf } from "./useKeyboardLighting";

export function LightBarPreview({ draft }: { draft: LightBarValues }) {
  const enabled = draft.kind === "x58" || draft.settings.enabled && draft.settings.brightness > 0;
  const color = draft.kind === "x170" ? draft.settings.color : X58_COLORS.find(color => color.value === String(draft.settings.colorId))?.rgb;
  return <div className="ccc-light-bar-preview" aria-label="灯条草稿示意">
    <svg viewBox="0 0 400 48" aria-hidden="true">{Array.from({ length: 12 }, (_, index) =>
      <rect key={index} x={index * 33 + 3} y="14" width="28" height="20" rx="4"
        fill={enabled ? color ? hexOf(color) : "var(--ccc-action)" : "var(--ccc-hairline)"}
        opacity={enabled ? 0.35 + draft.settings.brightness * 0.16 : 0.4} />)}</svg>
    <span className="ccc-control-caption">草稿示意 · 实际效果以设备为准</span>
  </div>;
}
export function X170LightBarEditor({ current, draft, presets, busy, onChange }: {
  current: X170LightBarSettings; draft: X170LightBarSettings; presets: X170LightBarSettings[]; busy: boolean; onChange: (value: X170LightBarSettings) => void;
}) {
  const update = (patch: Partial<X170LightBarSettings>) => onChange({ ...draft, ...patch });
  const colorSupported = ["static", "breath", "blink", "meteor"].includes(draft.effect);
  return <>
    <div className="ccc-panel-head"><strong>灯条草稿</strong><Switch label="独立灯条开关" checked={draft.enabled} disabled={busy} onChange={enabled => update({ enabled })} /></div>
    <LightBarPreview draft={{ kind: "x170", settings: draft }} />
    <Select label="独立灯条模式" value={draft.effect} options={X170_EFFECTS} disabled={busy} onChange={effect => {
      const preset = presets.find(value => value.effect === effect); if (preset) update({ effect: preset.effect, color: preset.color });
    }} />
    <ScaleSlider label="独立灯条亮度" unit="级" min={0} max={4} current={current.brightness} draft={draft.brightness} disabled={busy} onChange={brightness => update({ brightness })} />
    <ScaleSlider label="独立灯条速度" unit="档" min={1} max={3} current={current.speed + 1} draft={draft.speed + 1} disabled={busy} onChange={speed => update({ speed: speed - 1 })} />
    {colorSupported && draft.effect !== "static" && <Checkbox label="自定义独立灯条颜色" checked={draft.color !== null} disabled={busy}
      onChange={event => update({ color: event.target.checked ? { r: 0, g: 100, b: 255 } : null })} />}
    {colorSupported && draft.color && <label className="ccc-color-picker">灯条颜色
      <Input type="color" aria-label="独立灯条颜色" value={hexOf(draft.color)} disabled={busy} onChange={event => update({ color: rgbOf(event.target.value) })} /></label>}
  </>;
}
export function X58LightBarEditor({ current, draft, presets, busy, onChange }: {
  current: X58LightBarSettings; draft: X58LightBarSettings; presets: X58LightBarSettings[]; busy: boolean; onChange: (value: X58LightBarSettings) => void;
}) {
  const update = (patch: Partial<X58LightBarSettings>) => onChange({ ...draft, ...patch });
  const colorSupported = draft.effect === "static" || draft.effect === "scan";
  return <>
    <strong>灯条草稿</strong><LightBarPreview draft={{ kind: "x58", settings: draft }} />
    <Select label="独立灯条模式" value={draft.effect} options={X58_EFFECTS} disabled={busy} onChange={effect => {
      const mode = X58_EFFECTS.find(value => value.value === effect), preset = presets.find(value => value.effect === effect);
      if (mode) update({ effect: mode.value, colorId: preset?.colorId ?? null });
    }} />
    <ScaleSlider label="独立灯条亮度" unit="级" min={0} max={4} current={current.brightness} draft={draft.brightness} disabled={busy} onChange={brightness => update({ brightness })} />
    <p className="ccc-control-caption">0 级是原厂最低亮度档，未提供独立关灯命令。</p>
    {draft.effect !== "static" && <ScaleSlider label="独立灯条速度" unit="档" min={1} max={3} current={current.speed} draft={draft.speed} disabled={busy} onChange={speed => update({ speed })} />}
    {colorSupported && <Select label="独立灯条七色" value={draft.colorId === null ? "" : String(draft.colorId)} options={X58_COLORS} disabled={busy} onChange={colorId => update({ colorId: Number(colorId) })} />}
    {colorSupported && draft.colorId === null && <p className="ccc-control-caption">该模式的保存颜色未知，请先选择七色之一。</p>}
  </>;
}
