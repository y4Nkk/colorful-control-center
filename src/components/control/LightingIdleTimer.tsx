import { Input } from "../Input";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { IDLE_TIME_FIELDS, idleTimeFieldValid, type IdleTimerDraft } from "./lighting-option-values";

export function LightingIdleTimer({ timer, seconds, disabled, onChange, onToggle }: {
  timer: IdleTimerDraft; seconds: number | null; disabled: boolean;
  onChange: (timer: IdleTimerDraft) => void; onToggle: (enabled: boolean) => void;
}) {
  const invalid = seconds === null || (timer.enabled && seconds < 5);
  return <Surface as="div" variant="inset" className="ccc-lighting-option">
    <div className="ccc-keyboard-setting"><strong>闲置关灯</strong>
      <Switch label="启用闲置关灯" checked={timer.enabled} disabled={disabled} onChange={onToggle} />
    </div>
    <div className="ccc-lighting-time" role="group" aria-label="闲置等待时长">
      {IDLE_TIME_FIELDS.map(({ key, label, name, max }) => <label key={key}>
        <span>{label}</span>
        <Input type="number" size="compact" className="ccc-mono" aria-label={name} value={timer[key]} min={0} max={max} step={1}
          disabled={disabled || !timer.enabled} aria-invalid={!idleTimeFieldValid(timer[key], max) || (key === "seconds" && invalid && seconds !== null)}
          onChange={event => onChange({ ...timer, [key]: event.target.value === "" ? "" : event.target.valueAsNumber })} />
      </label>)}
    </div>
    {invalid && <p className="ccc-control-error" role="alert">{timer.enabled
      ? "请输入 5 秒到 1 小时 59 分 59 秒的等待时长。" : "启用闲置关灯后修正等待时长。"}</p>}
  </Surface>;
}
