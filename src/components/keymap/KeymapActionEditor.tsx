import type { KeymapAction, KeymapBinding, KeymapConfiguration, KeymapDevice } from "../../api/client";
import { KEY_OPTIONS, MOUSE_OPTIONS, optionsFor } from "../../lib/keymap";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { MacroRecorder } from "./MacroRecorder";
import { MacroStepsEditor } from "./MacroStepsEditor";
import { TextMacroEditor } from "./TextMacroEditor";

const ACTIONS = [{ value: "keys", label: "按键与鼠标宏" }, { value: "text", label: "发送文本" },
  { value: "launch", label: "启动程序" }, { value: "disable", label: "禁用按键" }];
function newAction(kind: string, configuration: KeymapConfiguration): KeymapAction {
  if (kind === "keys") return { kind, steps: [{ kind: "key", scanCode: 30, pressed: true, delayAfterMs: 0 }, { kind: "key", scanCode: 30, pressed: false, delayAfterMs: 0 }] };
  if (kind === "text") return { kind, text: "", before: [], after: [] };
  if (kind === "launch") {
    const used = new Set(configuration.devices.flatMap(device => device.bindings.flatMap(binding => binding.action.kind === "launch" ? [binding.action.eventId] : [])));
    let eventId = 256; while (used.has(eventId) && eventId < 65535) eventId++;
    return { kind, path: "", eventId };
  }
  return { kind: "disable" };
}
export function KeymapActionEditor({ active, binding, device, configuration, disabled, recording, onRecordingChange, onChange, onRemove }: {
  active: boolean; binding: KeymapBinding; device: KeymapDevice; configuration: KeymapConfiguration;
  disabled: boolean; recording: boolean; onRecordingChange: (active: boolean) => void;
  onChange: (binding: KeymapBinding) => void; onRemove: () => void;
}) {
  const action = binding.action, locked = disabled || recording;
  const group = configuration.devices.find(group => group.device === device);
  const options = (device === "keyboard" ? optionsFor(binding.trigger) : MOUSE_OPTIONS)
    .filter(key => Number(key.value) === binding.trigger || !group?.bindings.some(other => other.trigger === Number(key.value)));
  return <div className="ccc-keymap-editor">
    <div className="ccc-keymap-editor-heading"><strong>编辑映射</strong><Button variant="ghost" size="compact" disabled={locked} onClick={onRemove}>移除映射</Button></div>
    <label className="ccc-keymap-field">触发按键 <Select label="触发按键" value={String(binding.trigger)} options={options.length ? options : KEY_OPTIONS}
      disabled={locked} onChange={value => onChange({ ...binding, trigger: Number(value) })} /></label>
    <label className="ccc-keymap-field">执行动作 <Select label="执行动作" value={action.kind} options={ACTIONS} disabled={locked}
      onChange={kind => onChange({ ...binding, action: newAction(kind, configuration) })} /></label>
    {action.kind === "disable" && <span className="ccc-control-caption is-warn">应用后禁用此按键</span>}
    {action.kind === "keys" && <>
      <MacroRecorder active={active} disabled={disabled} onRecordingChange={onRecordingChange}
        onAdopt={steps => onChange({ ...binding, action: { kind: "keys", steps } })} />
      <MacroStepsEditor steps={action.steps} disabled={locked} onChange={steps => onChange({ ...binding, action: { kind: "keys", steps } })} />
    </>}
    {action.kind === "text" && <TextMacroEditor action={action} disabled={locked} onChange={action => onChange({ ...binding, action })} />}
    {action.kind === "launch" && <label className="ccc-keymap-field">程序完整路径 <Input aria-label="宏程序路径" value={action.path} disabled={locked}
      placeholder="C:\\Apps\\Application.exe" onChange={event => onChange({ ...binding, action: { ...action, path: event.target.value } })} /></label>}
  </div>;
}
