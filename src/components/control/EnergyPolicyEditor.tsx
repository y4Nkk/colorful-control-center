import type { EnergyPolicy, EnergyPolicyProfile, EnergySlot } from "../../api/client";
import { ENERGY_DAYS, ENERGY_FLOORS, ENERGY_CEILINGS, energyHour } from "../../lib/energy-policy";
import { Checkbox } from "../Checkbox";
import { Select } from "../Select";
import { Switch } from "../Switch";
import { EnergyTimeline } from "./EnergyTimeline";

function SlotEditor({ slot, index, disabled, onChange }: { slot: EnergySlot | null; index: number; disabled: boolean; onChange: (value: EnergySlot | null) => void }) {
  return <div className="ccc-energy-slot">
    <div className="ccc-keyboard-setting"><strong>时段 {index + 1}</strong><Switch label={`启用节能时段 ${index + 1}`} checked={!!slot} disabled={disabled}
      onChange={enabled => onChange(enabled ? { start: index === 0 ? 8 : 20, end: index === 0 ? 18 : 22 } : null)} /></div>
    {slot && <div className="ccc-battery-thresholds"><div><span className="ccc-control-caption">开始</span><Select label={`时段 ${index + 1} 开始`} value={String(slot.start)} disabled={disabled}
      options={Array.from({ length: 24 }, (_, i) => ({ value: String(i), label: energyHour(i) }))} onChange={value => onChange({ ...slot, start: Number(value) })} /></div>
      <div><span className="ccc-control-caption">结束</span><Select label={`时段 ${index + 1} 结束`} value={String(slot.end)} disabled={disabled}
        options={Array.from({ length: 24 }, (_, i) => ({ value: String(i + 1), label: energyHour(i + 1) }))} onChange={value => onChange({ ...slot, end: Number(value) })} /></div></div>}
  </div>;
}
export function EnergyPolicyEditor({ draft, profile, disabled, onChange, onAutomaticChange }: {
  draft: EnergyPolicy; profile: EnergyPolicyProfile; disabled: boolean;
  onChange: (policy: EnergyPolicy) => void; onAutomaticChange: (automatic: boolean) => void;
}) {
  return <div className="ccc-energy-editor">
    <div className="ccc-keyboard-setting"><strong>每天</strong><Switch label="每天执行节能时段" checked={draft.everyDay} disabled={disabled} onChange={everyDay => onChange({ ...draft, everyDay })} /></div>
    {!draft.everyDay && <div className="ccc-energy-days">{ENERGY_DAYS.map((day, index) => <Checkbox key={day} label={`周${day}`} aria-label={`节能星期${day}`} checked={!!(draft.weekdays & (1 << index))} disabled={disabled}
      onChange={event => onChange({ ...draft, weekdays: event.target.checked ? draft.weekdays | (1 << index) : draft.weekdays & ~(1 << index) })} />)}</div>}
    <div className="ccc-battery-thresholds ccc-energy-slots">{draft.slots.map((slot, index) => <SlotEditor key={index} slot={slot} index={index} disabled={disabled} onChange={value => {
      const slots = [...draft.slots] as EnergyPolicy["slots"]; slots[index] = value; onChange({ ...draft, slots });
    }} />)}</div>
    <EnergyTimeline slots={draft.slots} label="待应用时段" />
    <Select label="节能阈值方式" value={draft.automatic ? "auto" : "manual"} disabled={disabled}
      options={[{ value: "auto", label: `${profile.capability.factoryDefaulted ? "原厂默认" : "原厂自动"} · ${profile.capability.factory.floor}–${profile.capability.factory.ceiling}%` }, { value: "manual", label: "自定义阈值" }]}
      onChange={value => onAutomaticChange(value === "auto")} />
    {!draft.automatic && <div className="ccc-battery-thresholds"><div><span className="ccc-control-caption">放电下限</span><Select label="节能放电下限" value={String(draft.limits.floor)} disabled={disabled}
      options={ENERGY_FLOORS.map(n => ({ value: String(n), label: `${n}%` }))} onChange={value => onChange({ ...draft, limits: { ...draft.limits, floor: Number(value) } })} /></div>
      <div><span className="ccc-control-caption">充电上限</span><Select label="节能充电上限" value={String(draft.limits.ceiling)} disabled={disabled} options={ENERGY_CEILINGS.map(n => ({ value: String(n), label: `${n}%` }))}
        onChange={value => onChange({ ...draft, limits: { ...draft.limits, ceiling: Number(value) } })} /></div></div>}
  </div>;
}
