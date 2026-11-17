import type { MultiZoneValues, RgbColor } from "../../api/client";
import { Input } from "../Input";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ScaleSlider } from "./ScaleSlider";
import { MULTI_ZONE_MODES } from "./multizone-values";
import { hexOf, rgbOf } from "./useKeyboardLighting";

export function MultiZoneEditor({ current, draft, disabled, onChange }: {
  current: MultiZoneValues; draft: MultiZoneValues; disabled: boolean; onChange: (draft: MultiZoneValues) => void;
}) {
  const update = (patch: Partial<MultiZoneValues>) => onChange({ ...draft, ...patch });
  const color = (name: string, value: RgbColor, change: (color: RgbColor) => void) =>
    <label>{name}<Input aria-label={`多区${name}颜色`} type="color" value={hexOf(value)} disabled={disabled || draft.mode !== 1} onChange={event => change(rgbOf(event.target.value))} /></label>;
  return <>
    <div className="ccc-panel-head"><strong>多区草稿</strong><Switch label="多区键盘灯开关" checked={draft.enabled} disabled={disabled} onChange={enabled => update({ enabled })} /></div>
    <Select label="多区键盘模式" value={String(draft.mode)} options={MULTI_ZONE_MODES} disabled={disabled} onChange={mode => update({ mode: Number(mode) })} />
    <ScaleSlider label="多区亮度" unit="级" min={1} max={4} current={current.brightness} draft={draft.brightness} disabled={disabled} onChange={brightness => update({ brightness })} />
    <ScaleSlider label="多区速度" unit="级" min={1} max={3} current={current.speed} draft={draft.speed} disabled={disabled || draft.mode === 1} onChange={speed => update({ speed })} />
    <div className="ccc-multi-zone-inputs">{draft.zones.map((value, index) =>
      <div key={index}>{color(`第${index + 1}区`, value, color => {
        const zones = [...draft.zones] as MultiZoneValues["zones"]; zones[index] = color; update({ zones });
      })}</div>)}
      {draft.lightBar && color("灯条", draft.lightBar, lightBar => update({ lightBar }))}
      {draft.cover && color("顶盖", draft.cover, cover => update({ cover }))}
    </div>
    {draft.mode !== 1 && <p className="ccc-control-caption">切换到静态分区后可编辑颜色预设。</p>}
  </>;
}
