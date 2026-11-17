import type { DustSchedule } from "../../api/client";
import { Checkbox } from "../Checkbox";
import { Select } from "../Select";
import { Switch } from "../Switch";

export const DUST_DAYS = [1,2,3,4,5,6,0].map(day => ({ bit: 1 << day, label: `周${"日一二三四五六"[day]}` }));
export function formatDustSchedule(schedule: DustSchedule | null): string {
  if (!schedule) return "原厂未保存计划";
  const parts = [schedule.onResume ? "系统唤醒后" : "", schedule.weekdays ?
    `${schedule.weekdays === 127 ? "每天" : DUST_DAYS.filter(day => schedule.weekdays & day.bit).map(day => day.label).join("、")} ${String(schedule.hour).padStart(2,"0")}:00` : ""].filter(Boolean);
  return parts.length ? parts.join("；") : "定时和唤醒触发均关闭";
}
export function DustScheduleEditor({ draft, disabled, onChange }: {
  draft: DustSchedule; disabled: boolean; onChange: (value: DustSchedule) => void;
}) {
  return <div className="ccc-dust-editor">
    <div className="ccc-keyboard-setting"><span>系统唤醒后除尘</span>
      <Switch label="系统唤醒后除尘" checked={draft.onResume} disabled={disabled} onChange={onResume => onChange({ ...draft, onResume })} /></div>
    <div className="ccc-keyboard-setting"><span>按星期除尘</span>
      <Switch label="按星期除尘" checked={draft.weekdays !== 0} disabled={disabled}
        onChange={enabled => onChange({ ...draft, weekdays: enabled ? 127 : 0, hour: enabled ? draft.hour : 0 })} /></div>
    {draft.weekdays !== 0 && <>
      <div className="ccc-dust-days">{DUST_DAYS.map(day => <Checkbox key={day.bit} label={day.label} checked={!!(draft.weekdays & day.bit)} disabled={disabled}
        onChange={event => { const weekdays = event.target.checked ? draft.weekdays | day.bit : draft.weekdays & ~day.bit;
          onChange({ ...draft, weekdays, hour: weekdays ? draft.hour : 0 }); }} />)}</div>
      <label className="ccc-dust-hour">执行时间 <Select label="除尘执行时间" value={String(draft.hour)} disabled={disabled}
        options={Array.from({ length: 24 }, (_, hour) => ({ value: String(hour), label: `${String(hour).padStart(2,"0")}:00` }))}
        onChange={value => onChange({ ...draft, hour: Number(value) })} /></label>
    </>}
  </div>;
}
