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
export function DustScheduleEditor({ schedule, disabled, softwareActive, onChange }: {
  schedule: DustSchedule; disabled: boolean; softwareActive: boolean; onChange: (value: DustSchedule) => void;
}) {
  return <div className="ccc-dust-editor">
    <div className="ccc-keyboard-setting"><span>系统唤醒后除尘</span>
      <Switch label="系统唤醒后除尘" checked={schedule.onResume} disabled={disabled || softwareActive && (!schedule.onResume || schedule.weekdays !== 0)}
        onChange={onResume => onChange({ ...schedule, onResume })} /></div>
    <div className="ccc-keyboard-setting"><span>按星期除尘</span>
      <Switch label="按星期除尘" checked={schedule.weekdays !== 0} disabled={disabled || softwareActive && (!schedule.weekdays || schedule.onResume)}
        onChange={enabled => onChange({ ...schedule, weekdays: enabled ? 127 : 0, hour: enabled ? schedule.hour : 0 })} /></div>
    {schedule.weekdays !== 0 && <>
      <div className="ccc-dust-days">{DUST_DAYS.map(day => <Checkbox key={day.bit} label={day.label} checked={!!(schedule.weekdays & day.bit)} disabled={disabled || softwareActive}
        onChange={event => { const weekdays = event.target.checked ? schedule.weekdays | day.bit : schedule.weekdays & ~day.bit;
          onChange({ ...schedule, weekdays, hour: weekdays ? schedule.hour : 0 }); }} />)}</div>
      <label className="ccc-dust-hour">执行时间 <Select label="除尘执行时间" value={String(schedule.hour)} disabled={disabled || softwareActive}
        options={Array.from({ length: 24 }, (_, hour) => ({ value: String(hour), label: `${String(hour).padStart(2,"0")}:00` }))}
        onChange={value => onChange({ ...schedule, hour: Number(value) })} /></label>
    </>}
  </div>;
}
