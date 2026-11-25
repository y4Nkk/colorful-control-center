import { useId } from "react";
import { Switch } from "../Switch";
import { PERFORMANCE_OPTION_LABELS, type OemPerformanceOptions } from "../../api/client";
import type { CoolingChoice } from "./cooling-choice";
import { choiceDisabled, optionChoice } from "./cooling-choice";

export function CoolingOptions({ options, busy, disabled, custom, software, onChoice }: {
  options: OemPerformanceOptions; busy: boolean; disabled: boolean; custom: boolean; software: boolean;
  onChoice: (choice: CoolingChoice) => void;
}) {
  const { current, appliesInCurrentMode: applies, dttDriverAvailable: driver, currentMode } = options;
  const editable = currentMode === 2 || currentMode === 3;
  const disallowed = (choice: CoolingChoice) => choiceDisabled(options, choice, custom, software);
  const choices = (["turboFan", "dtt"] as const).map(key => {
    const saved = current[key];
    const active = key === "turboFan" ? options.turboActive : options.dttActive;
    const choice = optionChoice(options, key, !saved, custom);
    const status = saved === null ? "未确认支持" : active ? "已应用" : saved ? "仅保存" : null;
    return { key, saved, choice, status, savedOnly: saved === true && !active };
  });
  const quiet = currentMode === 3 ? options.quietFan : null;
  const quietChoice = quiet && optionChoice(options, "quietPreference", !quiet.enabled, custom);
  return <div className="ccc-cooling-options">
    <h4>预设选项</h4>
    <div className="ccc-cooling-option-list">
      {choices.map(({ key, saved, choice, status, savedOnly }) => <CoolingOption key={key} label={PERFORMANCE_OPTION_LABELS[key]}
        status={status} savedOnly={savedOnly} checked={saved === true} disabled={disabled || disallowed(choice)}
        busy={busy} onChange={() => onChoice(choice)} />)}
      {quiet && quietChoice && <CoolingOption label="娱乐档寂静偏好" status={quiet.enabled ? "已保存" : null}
        savedOnly checked={quiet.enabled} disabled={disabled || disallowed(quietChoice)} busy={busy}
        onChange={() => onChoice(quietChoice)} />}
    </div>
    {quiet?.enabled && !quiet.active && !options.turboActive &&
      <p className="ccc-control-caption">选择「自动」时会应用娱乐档寂静偏好。</p>}
    {!editable && <p className="ccc-control-caption">在娱乐或性能模式下可修改 Turbo 风扇和动态调校 DTT。</p>}
    {editable && !applies && choices.some(choice => choice.saved !== null) &&
      <p className="ccc-control-caption">此设备可在娱乐档保存 Turbo 风扇和 DTT 偏好，切换到性能档后应用。</p>}
    {applies && driver !== true && current.dtt !== null && <p className="ccc-control-caption">
      {driver === false ? "Dynamic Tuning 驱动未就绪，不能启用 DTT。" : "驱动状态未确认，不能启用 DTT。"}
      {options.dttActive && " 更改关联的风扇选项前，请确认驱动或关闭 DTT。"}
    </p>}
    {applies && currentMode === 2 && custom && current.turboFan !== null &&
      <p className="ccc-control-caption">退出自定义固件风扇曲线后可修改 Turbo 风扇。</p>}
    {software && (applies || quiet) && <p className="ccc-control-caption">在「风扇模式」选择「自动」后，可应用这些选项。</p>}
    {options.blockedReason && <p className="ccc-control-caption">{options.blockedReason}</p>}
  </div>;
}

function CoolingOption({ label, status, savedOnly, checked, disabled, busy, onChange }: {
  label: string; status: string | null; savedOnly: boolean; checked: boolean;
  disabled: boolean; busy: boolean; onChange: () => void;
}) {
  const statusId = useId();
  return <div className="ccc-cooling-option">
    <strong>{label}</strong>
    {status && <span id={statusId} hidden>{status}</span>}
    <Switch label={label} describedBy={status ? statusId : undefined} checked={checked} checkedTone={savedOnly ? "neutral" : undefined}
      disabled={disabled} busy={busy} onChange={onChange} />
  </div>;
}
