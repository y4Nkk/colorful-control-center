import type { KeymapAction, KeymapClick } from "../../api/client";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { Textarea } from "../Textarea";
import { optionsFor } from "../../lib/keymap";

function Clicks({ label, keys, disabled, onChange }: { label: string; keys: KeymapClick[]; disabled: boolean; onChange: (keys: KeymapClick[]) => void }) {
  return <div className="ccc-keymap-clicks"><strong>{label}</strong>
    {keys.map((key, index) => <div className="ccc-keymap-click" key={index}>
      <Select label={`${label}第${index + 1}键`} value={String(key.scanCode)} options={optionsFor(key.scanCode)} disabled={disabled}
        onChange={value => onChange(keys.map((item, at) => at === index ? { ...item, scanCode: Number(value) } : item))} />
      <label className="ccc-keymap-field">按下后 ms <Input type="number" aria-label={`${label}第${index + 1}键按下延时`} value={key.makeDelayMs} min={0} max={4294967295} disabled={disabled}
        onChange={event => onChange(keys.map((item, at) => at === index ? { ...item, makeDelayMs: Math.max(0, Math.min(4294967295, Math.round(Number(event.target.value) || 0))) } : item))} /></label>
      <label className="ccc-keymap-field">释放后 ms <Input type="number" aria-label={`${label}第${index + 1}键释放延时`} value={key.breakDelayMs} min={0} max={4294967295} disabled={disabled}
        onChange={event => onChange(keys.map((item, at) => at === index ? { ...item, breakDelayMs: Math.max(0, Math.min(4294967295, Math.round(Number(event.target.value) || 0))) } : item))} /></label>
      <Button icon="delete" variant="ghost" size="compact" aria-label={`删除${label}第${index + 1}键`} disabled={disabled} onClick={() => onChange(keys.filter((_, at) => at !== index))}>删除</Button>
    </div>)}
    <Button icon="add" variant="secondary" size="compact" disabled={disabled || keys.length >= 6} onClick={() => onChange([...keys, { scanCode: 28, makeDelayMs: 0, breakDelayMs: 0 }])}>添加{label}</Button>
  </div>;
}
export function TextMacroEditor({ action, disabled, onChange }: { action: Extract<KeymapAction, { kind: "text" }>; disabled: boolean; onChange: (action: KeymapAction) => void }) {
  return <div className="ccc-keymap-text">
    <label className="ccc-keymap-field">发送文本 <Textarea aria-label="文本宏内容" value={action.text} disabled={disabled} onChange={event => onChange({ ...action, text: event.target.value })} /></label>
    <p className="ccc-control-caption">通过原厂剪贴板服务发送；此处编辑只修改草稿。</p>
    <Clicks label="前置按键" keys={action.before} disabled={disabled} onChange={before => onChange({ ...action, before })} />
    <Clicks label="后置按键" keys={action.after} disabled={disabled} onChange={after => onChange({ ...action, after })} />
  </div>;
}
