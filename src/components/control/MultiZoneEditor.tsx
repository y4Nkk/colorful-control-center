import type { MultiZoneValues } from "../../api/client";
import { ColorPicker } from "../ColorPicker";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { ScaleSlider } from "./ScaleSlider";
import { MULTI_ZONE_MODES } from "./multizone-values";

export function MultiZoneEditor({ current, draft, disabled, onChange, onApply }: {
  current: MultiZoneValues; draft: MultiZoneValues; disabled: boolean; onChange: (draft: MultiZoneValues) => void;
  onApply: (patch: Partial<MultiZoneValues>) => void;
}) {
  const update = (patch: Partial<MultiZoneValues>) => onChange({ ...draft, ...patch });
  const colorDisabled = disabled || draft.mode !== 1;
  return <>
    <div className="ccc-panel-head"><strong>分区灯光</strong><Switch label="多区键盘灯开关" checked={current.enabled} disabled={disabled} onChange={enabled => onApply({ enabled })} /></div>
    <Select label="多区键盘模式" value={String(current.mode)} options={MULTI_ZONE_MODES} disabled={disabled} onChange={mode => onApply({ mode: Number(mode) })} />
    <ScaleSlider label="多区亮度" unit="级" min={1} max={4} current={current.brightness} draft={draft.brightness} disabled={disabled} onChange={brightness => update({ brightness })} />
    <ScaleSlider label="多区速度" unit="级" min={1} max={3} current={current.speed} draft={draft.speed} disabled={disabled || draft.mode === 1} onChange={speed => update({ speed })} />
    <div className="ccc-multi-zone-inputs">{draft.zones.map((value, index) =>
      <ColorPicker key={index} label={`第${index + 1}区颜色`} value={value} disabled={colorDisabled} onChange={color => {
        const zones = [...draft.zones] as MultiZoneValues["zones"]; zones[index] = color; update({ zones });
      }} />)}
      {draft.lightBar && <ColorPicker label="多区灯条颜色" value={draft.lightBar} disabled={colorDisabled} onChange={lightBar => update({ lightBar })} />}
      {draft.cover && <ColorPicker label="多区顶盖颜色" value={draft.cover} disabled={colorDisabled} onChange={cover => update({ cover })} />}
    </div>
    {draft.mode !== 1 && <p className="ccc-control-caption">切换到静态分区后可编辑颜色预设。</p>}
  </>;
}
