import { Icon } from "../Icon";
import { applyBatteryCharge, restoreBatteryCharge, type BatteryChargeValues } from "../../api/client";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { BatteryOverview } from "./BatteryOverview";
import { useBatterySettings } from "./useBatterySettings";
import { useEffect, useState } from "react";
import { EnergyPolicySettings } from "./EnergyPolicySettings";
import { BatteryCalibrationSettings } from "./BatteryCalibrationSettings";

const description = (value: BatteryChargeValues) => value.enabled ? `低于 ${value.start}% 开始，达到 ${value.stop}% 停止` : "充至满电";
const choices = (values: number[], current: number) => [...new Set([...values, current])].map(value => ({ value: String(value), label: value === 0 ? "未设置" : `${value}%` }));

export function BatterySettings({ active, revision: refreshRevision, onPolicyChanged }: { active: boolean; revision: number; onPolicyChanged: () => void }) {
  const { profile, draft, setDraft, busy: chargeBusy, error, run, reread, refreshFacts } = useBatterySettings(active, refreshRevision);
  const [energyBusy, setEnergyBusy] = useState(false);
  const [calibrationBusy, setCalibrationBusy] = useState(false), [calibrationLocked, setCalibrationLocked] = useState(false);
  useEffect(onPolicyChanged, [calibrationLocked, onPolicyChanged]);
  const busy = chargeBusy || energyBusy || calibrationBusy;
  const charge = profile?.charge, power = profile?.power, diagnostics = profile?.diagnostics;
  const dirty = !!charge && !!draft && JSON.stringify(charge.current) !== JSON.stringify(draft);
  const valid = !!draft && (!draft.enabled && draft.start === charge?.current.start && draft.stop === charge.current.stop
    || !!charge && charge.startValues.includes(draft.start) && charge.stopValues.includes(draft.stop) && draft.start < draft.stop);
  const update = (patch: Partial<BatteryChargeValues>) => setDraft(previous => previous ? { ...previous, ...patch } : previous);
  const canWrite = power?.present === true && !!charge && !!profile?.revision && !profile.chargeLockReason && !calibrationLocked;
  const revision = profile?.revision;
  return <Surface as="section" variant="group" className="ccc-panel ccc-battery-panel" aria-label="电池与充电">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="charging" />电池与充电</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取电池</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <BatteryOverview profile={profile} />
    {!profile ? <p className="ccc-control-caption">{error ? "暂无有效电池读数，请重新读取。" : "正在读取电池。"}</p> : !charge || !draft ?
      <p className="ccc-control-caption">暂未读取到可用的充电保护接口。</p> : <>
      <Surface as="div" variant="inset" className="ccc-keyboard-setting">
        <strong>限制充电范围</strong>
        <Switch label="限制充电范围" checked={draft.enabled} disabled={busy || !canWrite} onChange={enabled => update({ enabled })} />
      </Surface>
      <div className="ccc-battery-thresholds">
        <div><span className="ccc-control-caption">开始充电</span><Select label="开始充电阈值" value={String(draft.start)} options={choices(charge.startValues, draft.start)} disabled={busy || !canWrite || !draft.enabled}
          onChange={value => update({ start: Number(value) })} /></div>
        <div><span className="ccc-control-caption">停止充电</span><Select label="停止充电阈值" value={String(draft.stop)} options={choices(charge.stopValues, draft.stop)} disabled={busy || !canWrite || !draft.enabled}
          onChange={value => update({ stop: Number(value) })} /></div>
      </div>
      <p className="ccc-control-caption" role="status" aria-label="充电设置预览">当前：{description(charge.current)}{dirty ? ` → ${description(draft)}` : ""}</p>
      {!valid && <p className="ccc-control-error">请选择可用阈值，且开始值低于停止值。</p>}
      {power?.present !== true && <p className="ccc-control-caption">未确认电池存在，请重新读取后操作。</p>}
      {profile.chargeLockReason && <p className="ccc-control-caption">{profile.chargeLockReason}</p>}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify([revision, draft])} label="应用充电设置" confirmLabel="确认应用充电设置？" busy={busy} disabled={!canWrite || !valid || !dirty}
          onConfirm={() => revision ? run(() => applyBatteryCharge(revision, draft), "充电设置已应用") : undefined} />
        <ConfirmButton icon="undo" key={`restore-${revision}`} variant="secondary" label="恢复充电初始" confirmLabel="恢复本会话修改前的充电设置？" busy={busy} disabled={!canWrite || !profile.canRestore}
          onConfirm={() => revision ? run(() => restoreBatteryCharge(revision), "充电初始设置已恢复") : undefined} />
      </div>
    </>}
    <EnergyPolicySettings active={active} revision={refreshRevision} locked={chargeBusy || calibrationBusy || calibrationLocked || power?.present !== true} onChanged={refreshFacts} onBusy={setEnergyBusy} />
    <BatteryCalibrationSettings active={active} locked={chargeBusy || energyBusy} onChanged={refreshFacts} onBusy={setCalibrationBusy} onLocked={setCalibrationLocked} />
    <Collapsible summary="电池读数说明与诊断">
      <p>满充与设计容量的比例用于比较容量变化，不代表当前电量或厂商健康诊断。充电设置生效后不会立即改变电量。</p>
      {diagnostics && <FactList compact facts={[
        { label: "生产日期", value: diagnostics.manufacturedOn ?? "—" },
        { label: "满充容量原始值", value: diagnostics.fullCapacityRaw },
        { label: "设计容量原始值", value: diagnostics.designCapacityRaw },
        { label: "电池状态码", value: `0x${diagnostics.batteryStatus.toString(16)}` },
        { label: "故障状态码", value: `0x${diagnostics.permanentFailureStatus.toString(16)}` },
      ]} />}
      {profile?.warnings.map(warning => <p key={warning}>{warning}</p>)}
    </Collapsible>
  </Surface>;
}
