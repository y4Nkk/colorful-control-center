import type { LightBarValues, X170LightBarSettings, X58LightBarSettings } from "../../api/client";
import { useState } from "react";
import { hexOf } from "../../lib/color";
import { Checkbox } from "../Checkbox";
import { ColorPicker } from "../ColorPicker";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ScaleSlider } from "./ScaleSlider";
import { X170_EFFECTS, X58_EFFECTS, X58_COLORS } from "./lightbar-values";

export function LightBarPreview({ draft }: { draft: LightBarValues }) {
  const enabled = draft.kind === "x58" || draft.settings.enabled && draft.settings.brightness > 0;
  const color = draft.kind === "x170" ? draft.settings.color : X58_COLORS.find(color => color.value === String(draft.settings.colorId))?.rgb;
  return <div className="ccc-light-bar-preview" aria-label="灯条草稿示意">
    <svg viewBox="0 0 400 48" aria-hidden="true">{Array.from({ length: 12 }, (_, index) =>
      <rect key={index} x={index * 33 + 3} y="14" width="28" height="20" rx="4"
        fill={enabled ? color ? hexOf(color) : "var(--ccc-action-ink)" : "var(--ccc-hairline)"}
        opacity={enabled ? 0.35 + draft.settings.brightness * 0.16 : 0.4} />)}</svg>
    <span className="ccc-control-caption">草稿示意 · 实际效果以设备为准</span>
  </div>;
}
export function X170LightBarEditor({ current, draft, presets, busy, blocked, onChange, onApply }: {
  current: X170LightBarSettings; draft: X170LightBarSettings; presets: X170LightBarSettings[]; busy: boolean; onChange: (value: X170LightBarSettings) => void;
  blocked: boolean; onApply: (value: X170LightBarSettings) => void;
}) {
  const update = (patch: Partial<X170LightBarSettings>) => onChange({ ...draft, ...patch });
  const colorSupported = ["static", "breath", "blink", "meteor"].includes(draft.effect);
  return <>
    <div className="ccc-panel-head"><strong>灯条预览</strong><Switch label="独立灯条开关" checked={current.enabled} disabled={blocked}
      onChange={enabled => onApply({ ...current, enabled })} /></div>
    <LightBarPreview draft={{ kind: "x170", settings: draft }} />
    <Select label="独立灯条模式" value={current.effect} options={X170_EFFECTS.map(option => ({ ...option, disabled: !presets.some(value => value.effect === option.value) }))}
      disabled={blocked} onChange={effect => {
      const preset = presets.find(value => value.effect === effect);
      if (preset) onApply({ ...current, effect: preset.effect, color: preset.color });
    }} />
    <ScaleSlider label="独立灯条亮度" unit="级" min={0} max={4} current={current.brightness} draft={draft.brightness} disabled={busy} onChange={brightness => update({ brightness })} />
    <ScaleSlider label="独立灯条速度" unit="档" min={1} max={3} current={current.speed + 1} draft={draft.speed + 1} disabled={busy} onChange={speed => update({ speed: speed - 1 })} />
    {colorSupported && draft.effect !== "static" && <Checkbox label="自定义独立灯条颜色" checked={current.color !== null} disabled={blocked}
      onChange={event => onApply({ ...current, color: event.target.checked ? { r: 0, g: 100, b: 255 } : null })} />}
    {colorSupported && draft.color && <ColorPicker label="独立灯条颜色" value={draft.color} disabled={busy} onChange={color => update({ color })} />}
  </>;
}
export function X58LightBarEditor({ current, draft, presets, busy, blocked, revision, onChange, onApply }: {
  current: X58LightBarSettings; draft: X58LightBarSettings; presets: X58LightBarSettings[]; busy: boolean; onChange: (value: X58LightBarSettings) => void;
  blocked: boolean; revision: string; onApply: (value: X58LightBarSettings) => void;
}) {
  const [pending, setPending] = useState<{ revision: string; effect: X58LightBarSettings["effect"] } | null>(null);
  const pendingEffect = pending?.revision === revision ? pending.effect : null;
  const update = (patch: Partial<X58LightBarSettings>) => onChange({ ...draft, ...patch });
  const effect = pendingEffect ?? current.effect;
  const colorSupported = effect === "static" || effect === "scan";
  return <>
    <strong>灯条预览</strong><LightBarPreview draft={{ kind: "x58", settings: draft }} />
    <Select label="独立灯条模式" value={effect} options={X58_EFFECTS} disabled={blocked} onChange={value => {
      const mode = X58_EFFECTS.find(option => option.value === value)?.value;
      if (!mode) return;
      const preset = presets.find(option => option.effect === mode);
      if (!preset && mode !== current.effect && (mode === "static" || mode === "scan")) {
        setPending({ revision, effect: mode });
        return;
      }
      setPending(null);
      if (mode !== current.effect) onApply({ ...current, effect: mode, colorId: preset?.colorId ?? null });
    }} />
    <ScaleSlider label="独立灯条亮度" unit="级" min={0} max={4} current={current.brightness} draft={draft.brightness} disabled={busy || !!pendingEffect} onChange={brightness => update({ brightness })} />
    <p className="ccc-control-caption">0 级是原厂最低亮度档，未提供独立关灯命令。</p>
    {effect !== "static" && <ScaleSlider label="独立灯条速度" unit="档" min={1} max={3} current={current.speed} draft={draft.speed}
      disabled={busy || !!pendingEffect} onChange={speed => update({ speed })} />}
    {colorSupported && <Select label="独立灯条七色" value={pendingEffect || current.colorId === null ? "" : String(current.colorId)}
      options={X58_COLORS} disabled={blocked} onChange={colorId => {
        setPending(null);
        onApply({ ...current, effect, colorId: Number(colorId) });
      }} />}
    {pendingEffect && <p className="ccc-control-caption">选择颜色后切换至该模式。</p>}
  </>;
}
