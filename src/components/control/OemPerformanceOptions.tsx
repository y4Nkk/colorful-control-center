import { useEffect, useState } from "react";
import { Icon } from "../Icon";
import { applyOemPerformanceOptions, readOemPerformanceOptions, NOISELESS_FAN_LABEL, OEM_POWER_MODES, PERFORMANCE_OPTION_LABELS,
  type OemPerformanceDraft, type OemPerformanceOptions as PerformanceProfile } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { Dialog } from "../Dialog";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { useDeviceSettings } from "./useDeviceSettings";
export const performanceState = (value: boolean | null) => value === null ? "未确认支持" : value ? "开启" : "关闭";
export const PERFORMANCE_FIELDS = [{ key: "turboFan", label: PERFORMANCE_OPTION_LABELS.turboFan, description: "启用固件的 Turbo 风扇策略" },
  { key: "dtt", label: PERFORMANCE_OPTION_LABELS.dtt, description: "使用 Dynamic Tuning 驱动调节性能" }] as const;
const QUIET_FAN_FIELD = { key: "quietFan", label: NOISELESS_FAN_LABEL, description: "娱乐与性能可分别开启固件的低噪声风扇策略" } as const;
type OptionChange = {
  revision: string; key: keyof OemPerformanceDraft; next: OemPerformanceDraft; replaceCustomFan: boolean;
};

export function OemPerformanceOptions({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readOemPerformanceOptions, active, revision);
  const [pending, setPending] = useState<OptionChange | null>(null);
  useEffect(() => setPending(null), [active, profile?.revision, profile?.softwareActive, profile?.blockedReason]);
  const current = profile && { ...profile.current, quietFan: profile.quietFan?.enabled ?? null };
  const fields = profile?.quietFan ? [...PERFORMANCE_FIELDS, QUIET_FAN_FIELD] : PERFORMANCE_FIELDS;
  const performance = profile?.currentMode === 2;
  const entertainment = profile?.currentMode === 3;
  const applies = profile?.appliesInCurrentMode === true;
  const canEdit = !busy && !profile?.blockedReason && (entertainment || performance) && (!applies || !profile?.softwareActive);
  const driverMissing = applies && profile?.dttDriverAvailable !== true;
  const disabled = (key: keyof OemPerformanceDraft) => !canEdit || current?.[key] == null
    || key === "quietFan" && !!profile?.softwareActive || applies && key === "turboFan" && !!profile?.customFan && !entertainment
    || driverMissing && (key === "dtt" && current.dtt !== true || current.dtt === true &&
      (key === "turboFan" || key === "quietFan" && (profile?.entertainmentOptionsPending || current.turboFan === true && current.quietFan === false)));
  const submit = ({ revision: expectedRevision, key, next, replaceCustomFan }: OptionChange) => {
    void run(async () => {
      const result = await applyOemPerformanceOptions(expectedRevision, next, replaceCustomFan);
      if (!result.ok) throw new Error(result.message);
    }, entertainment && !applies && key !== "quietFan" ? "性能偏好已保存" : "性能选项已回读").then(onSettled);
  };
  const toggle = (key: keyof OemPerformanceDraft, enabled: boolean, applySaved = false) => {
    const reapplyQuiet = key === "quietFan" && enabled && !profile?.quietFan?.active;
    if (!profile || !current || disabled(key) || current[key] === enabled && !reapplyQuiet && !applySaved) return;
    const next = { ...current, [key]: enabled, quietFan: key === "quietFan" ? enabled : null };
    if (applies && enabled && key === "quietFan" && next.turboFan !== null) next.turboFan = false;
    if (applies && enabled && key === "turboFan" && current.quietFan !== null) next.quietFan = false;
    if (driverMissing && next.dtt === true && (profile.entertainmentOptionsPending || next.dtt !== current.dtt || next.turboFan !== current.turboFan)) return;
    const replaceCustomFan = profile.customFan && (key === "quietFan" || entertainment && applies && key === "turboFan" && enabled);
    const change = { revision: profile.revision, key, next, replaceCustomFan };
    if (replaceCustomFan || entertainment && applies) setPending(change);
    else submit(change);
  };
  const savedKey = current?.turboFan ? "turboFan" : "dtt";
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="性能选项">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="performance" />性能选项</h2>
      <div className="ccc-module-tools"><DetailsDialog title="性能选项说明与诊断" label="详情">
        {fields.map(field => <p key={field.key}><strong>{field.label}</strong> · {field.description}</p>)}
        {applies && profile?.quietFan && current?.turboFan != null && <p className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{NOISELESS_FAN_LABEL}与 {PERFORMANCE_OPTION_LABELS.turboFan}互斥。</p>}
        {profile?.warnings.map(warning => <p key={warning} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
        <p>可在「预设模式」查看和恢复初始设置。</p>
      </DetailsDialog><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取性能选项" disabled={busy} onClick={reread} /></div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <OemConflictAction issues={[error, profile?.blockedReason]} disabled={busy} onClosed={reread} />
    {!profile || !current ? <p className="ccc-control-caption">{error ? "暂时无法读取，请重试。" : "正在读取性能选项。"}</p> : <>
      <div className="ccc-setting-tiles">{fields.map(({ key, label }) => <Surface as="div" variant="inset" className="ccc-keyboard-setting" key={key}>
        <div><strong>{label}</strong>
          {current[key] === null && <p className="ccc-control-caption">{performanceState(current[key])}</p>}
          {key === "quietFan" && <p className="ccc-control-caption">{OEM_POWER_MODES[profile.currentMode]}模式</p>}
        </div>
        <Switch label={label} checked={current[key] ?? false} disabled={disabled(key)} onChange={value => toggle(key, value)} />
      </Surface>)}</div>
      <PerformanceHints profile={profile} quietDisabled={disabled("quietFan") || driverMissing && !!current.dtt && !!current.turboFan}
        onApplyQuiet={() => toggle("quietFan", true)} />
      {profile.entertainmentOptionsPending && <Button className="ccc-inline-tool" variant="secondary" size="compact"
        disabled={disabled(savedKey) || driverMissing && current.dtt === true} onClick={() => toggle(savedKey, true, true)}>应用已保存的性能选项</Button>}
      {driverMissing && current.dtt !== null && <p className="ccc-control-caption">{profile.dttDriverAvailable === false ? "Dynamic Tuning 驱动未就绪，不能启用 DTT。" : "驱动状态未确认，不能启用 DTT。"}</p>}
      {driverMissing && current.dtt && <p className="ccc-control-caption">更改关联的风扇选项前，请确认驱动或关闭 DTT。</p>}
      {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    </>}
    {active && pending && profile && pending.revision === profile.revision && !disabled(pending.key) && <OptionConfirmation pending={pending}
      mode={OEM_POWER_MODES[profile.currentMode]} previewOptions={entertainment && applies} quietEnabled={current?.quietFan === true} onCancel={() => setPending(null)}
      onConfirm={() => { setPending(null); submit(pending); }} />}
  </Surface>;
}

function OptionConfirmation({ pending, mode, previewOptions, quietEnabled, onCancel, onConfirm }: {
  pending: OptionChange; mode: string; previewOptions: boolean; quietEnabled: boolean; onCancel: () => void; onConfirm: () => void;
}) {
  const quiet = pending.key === "quietFan";
  return <Dialog title={quiet ? "切换风扇策略" : "应用娱乐模式性能选项"} width="narrow" open onClose={onCancel} actions={<>
    <Button variant="secondary" onClick={onCancel}>取消</Button>
    <Button onClick={onConfirm}>{quiet ? "确认切换" : "确认应用"}</Button>
  </>}>
    {quiet ? <p>{mode}模式将{pending.replaceCustomFan ? "从自定义固件风扇曲线切换到" : "使用"}「{pending.next.quietFan ? NOISELESS_FAN_LABEL : "自动"}」。</p>
      : <p>在娱乐模式下应用以下设置。</p>}
    {previewOptions && PERFORMANCE_FIELDS.filter(({ key }) => pending.next[key] !== null).map(({ key, label }) =>
      <p key={key}>{label} · {performanceState(pending.next[key])}</p>)}
    {!quiet && (pending.key === "turboFan" || pending.next.turboFan) && <p>
      {pending.replaceCustomFan ? "将从自定义固件风扇曲线切换到「自动」，再启用 Turbo 风扇。" : "Turbo 风扇状态改变时，风扇策略将切换到「自动」。"}
      {pending.next.turboFan && quietEnabled ? `开启 Turbo 风扇会关闭${NOISELESS_FAN_LABEL}。` : ""}
    </p>}
  </Dialog>;
}

function PerformanceHints({ profile, quietDisabled, onApplyQuiet }: {
  profile: PerformanceProfile; quietDisabled: boolean; onApplyQuiet: () => void;
}) {
  const { quietFan, customFan, softwareActive, current, currentMode, appliesInCurrentMode } = profile;
  const performance = currentMode === 2, entertainment = currentMode === 3;
  const preferences = entertainment && !appliesInCurrentMode ? PERFORMANCE_FIELDS.filter(({ key }) => current[key] !== null).map(({ label }) => label) : [];
  const quietPending = quietFan?.enabled && (!quietFan.active || softwareActive);
  return <>
    {preferences.length > 0 && <p className="ccc-control-caption">{preferences.join("、")} 的偏好可在娱乐模式保存，切换到性能模式后生效。</p>}
    {!performance && !entertainment && <p className="ccc-control-caption">在娱乐或性能模式下可修改性能选项。</p>}
    {performance && quietFan && !quietFan.remembered && <p className="ccc-control-caption">切换模式后，此设备会重置当前模式的{NOISELESS_FAN_LABEL}设置。</p>}
    {quietPending && <div className="ccc-control-status">
      <span className="ccc-control-caption">{NOISELESS_FAN_LABEL}已保存为开启。{softwareActive ? "当前使用软件风扇曲线。" : customFan ? "当前使用自定义固件风扇曲线。" : "当前使用其他风扇档位。"}</span>
      {!softwareActive && <Button className="ccc-inline-tool" variant="secondary" size="compact" disabled={quietDisabled} onClick={onApplyQuiet}>应用已保存的{NOISELESS_FAN_LABEL}</Button>}
    </div>}
    {quietFan?.active && !quietFan.enabled && !softwareActive && <p className="ccc-control-caption">当前风扇档位为{NOISELESS_FAN_LABEL}。</p>}
    {appliesInCurrentMode && current.turboFan !== null && customFan && !softwareActive && <p className="ccc-control-caption">
      {entertainment ? "开启 Turbo 风扇前可预览并确认切换到自动风扇。" : "退出自定义固件风扇曲线后可修改 Turbo 风扇。"}
    </p>}
    {(appliesInCurrentMode || quietFan) && softwareActive && <p className="ccc-control-caption">退出软件风扇曲线后可修改{appliesInCurrentMode ? "性能选项" : NOISELESS_FAN_LABEL}。</p>}
  </>;
}
