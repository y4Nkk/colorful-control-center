import type { KeymapBinding, KeymapDeviceSettings } from "../../api/client";
import { actionLabel, keyLabel, KEY_OPTIONS, MOUSE_OPTIONS, KEYMAP_DEVICE_LABELS } from "../../lib/keymap";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";

export function KeymapBindingList({ group, selected, disabled, onSelect, onChange }: {
  group: KeymapDeviceSettings; selected: number | null; disabled: boolean;
  onSelect: (trigger: number) => void; onChange: (next: KeymapDeviceSettings) => void;
}) {
  const available = (group.device === "keyboard" ? KEY_OPTIONS : MOUSE_OPTIONS)
    .filter(key => !group.bindings.some(binding => binding.trigger === Number(key.value)));
  const add = () => {
    const trigger = Number(available[0]?.value); if (!trigger) return;
    const binding: KeymapBinding = { trigger, action: { kind: "disable" } };
    onChange({ ...group, bindings: [...group.bindings, binding] }); onSelect(trigger);
  };
  return <div className="ccc-keymap-bindings">
    <Checkbox label={`启用${KEYMAP_DEVICE_LABELS[group.device]}映射`} checked={group.enabled} disabled={disabled}
      onChange={event => onChange({ ...group, enabled: event.target.checked })} />
    <div className="ccc-keymap-list" aria-label={`${KEYMAP_DEVICE_LABELS[group.device]}映射列表`}>
      {group.bindings.map(binding => <Button key={binding.trigger} variant="selection" className="ccc-keymap-binding"
        aria-pressed={selected === binding.trigger} disabled={disabled} onClick={() => onSelect(binding.trigger)}>
        <strong>{keyLabel(group.device, binding.trigger)}</strong><span>{actionLabel(binding.action)}</span>
      </Button>)}
      {!group.bindings.length && <p className="ccc-control-caption">尚未配置映射，所有按键保持原操作。</p>}
    </div>
    <Button icon="add" variant="secondary" size="compact" disabled={disabled || !available.length} onClick={add}>添加映射</Button>
  </div>;
}
