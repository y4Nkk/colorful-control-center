import type { KeymapStep } from "../../api/client";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { MOUSE_OPTIONS, optionsFor, stepError } from "../../lib/keymap";

const delay = (value: string) => Math.max(0, Math.min(4294967295, Math.round(Number(value) || 0)));
export function MacroStepsEditor({ steps, disabled, onChange }: { steps: KeymapStep[]; disabled: boolean; onChange: (steps: KeymapStep[]) => void }) {
  const change = (index: number, step: KeymapStep) => onChange(steps.map((value, at) => at === index ? step : value));
  const move = (index: number, offset: number) => { const next = [...steps]; [next[index], next[index + offset]] = [next[index + offset], next[index]]; onChange(next); };
  return <div className="ccc-keymap-steps">
    {steps.map((step, index) => <Surface key={index} as="div" variant="inset" className="ccc-keymap-step">
      <div className="ccc-keymap-step-heading"><strong>第 {index + 1} 步</strong><div className="ccc-actions">
        <Button variant="ghost" size="compact" disabled={disabled || index === 0} aria-label={`上移第${index + 1}步`} onClick={() => move(index, -1)}>上移</Button>
        <Button variant="ghost" size="compact" disabled={disabled || index === steps.length - 1} aria-label={`下移第${index + 1}步`} onClick={() => move(index, 1)}>下移</Button>
        <Button icon="delete" variant="ghost" size="compact" disabled={disabled} aria-label={`删除第${index + 1}步`} onClick={() => onChange(steps.filter((_, at) => at !== index))}>删除</Button>
      </div></div>
      {step.kind === "key" ? <>
        <label className="ccc-keymap-field">按键 <Select label={`第${index + 1}步按键`} value={String(step.scanCode)} options={optionsFor(step.scanCode)} disabled={disabled}
          onChange={value => change(index, { ...step, scanCode: Number(value) })} /></label>
        <label className="ccc-keymap-field">动作 <Select label={`第${index + 1}步动作`} value={String(step.pressed)} options={[{ value: "true", label: "按下" }, { value: "false", label: "释放" }]} disabled={disabled}
          onChange={value => change(index, { ...step, pressed: value === "true" })} /></label>
        <label className="ccc-keymap-field">随后延时 ms <Input type="number" aria-label={`第${index + 1}步延时`} min={0} max={4294967295} step={1} value={step.delayAfterMs} disabled={disabled}
          onChange={event => change(index, { ...step, delayAfterMs: delay(event.target.value) })} /></label>
      </> : <>
        <label className="ccc-keymap-field">鼠标按键 <Select label={`第${index + 1}步鼠标按键`} value={String(step.button)} options={MOUSE_OPTIONS} disabled={disabled} onChange={value => change(index, { ...step, button: Number(value) })} /></label>
        <label className="ccc-keymap-field">按下后延时 ms <Input type="number" aria-label={`第${index + 1}步按下延时`} min={0} max={4294967295} value={step.makeDelayMs} disabled={disabled}
          onChange={event => change(index, { ...step, makeDelayMs: delay(event.target.value) })} /></label>
        <label className="ccc-keymap-field">释放后延时 ms <Input type="number" aria-label={`第${index + 1}步释放延时`} min={0} max={4294967295} value={step.breakDelayMs} disabled={disabled}
          onChange={event => change(index, { ...step, breakDelayMs: delay(event.target.value) })} /></label>
      </>}
    </Surface>)}
    <div className="ccc-actions">
      <Button icon="add" variant="secondary" size="compact" disabled={disabled || steps.length > 254} onClick={() => onChange([...steps,
        { kind: "key", scanCode: 30, pressed: true, delayAfterMs: 0 }, { kind: "key", scanCode: 30, pressed: false, delayAfterMs: 0 }])}>添加按键</Button>
      <Button icon="add" variant="secondary" size="compact" disabled={disabled || steps.length >= 256} onClick={() => onChange([...steps,
        { kind: "mouseClick", button: 1, makeDelayMs: 0, breakDelayMs: 0 }])}>添加鼠标点击</Button>
    </div>
    {stepError(steps) && <p className="ccc-control-caption">{stepError(steps)}</p>}
  </div>;
}
