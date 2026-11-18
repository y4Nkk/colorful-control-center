import { Icon } from "../Icon";
import { applyBatteryCharge, restoreBatteryCharge, type BatteryChargeValues } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
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
  return <div className="ccc-battery-workflows">
  <Surface as="section" variant="group" className="ccc-panel ccc-battery-panel" aria-label="电池与充电">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="charging" />电池与充电</h2>
      <div className="ccc-module-tools">
      <DetailsDialog title="电池读数与诊断" label="详情">
        {diagnostics && <FactList compact facts={[
          { label: "生产日期", value: diagnostics.manufacturedOn ?? "—" },
          { label: "循环次数", value: diagnostics.cycleCount ?? "—" },
          { label: "满充 / 设计容量", value: diagnostics.capacityPct === null ? "未知" : `${diagnostics.capacityPct}%` },
          { label: "满充容量原始值", value: diagnostics.fullCapacityRaw },
          { label: "设计容量原始值", value: diagnostics.designCapacityRaw },
          { label: "电池状态码", value: `0x${diagnostics.batteryStatus.toString(16)}` },
          { label: "故障状态码", value: `0x${diagnostics.permanentFailureStatus.toString(16)}` },
        ]} />}
        <p>满充与设计容量的比例用于比较容量变化。图中的当前电量取系统读数。</p>
        {profile?.warnings.map(warning => <p key={warning}>{warning}</p>)}
      </DetailsDialog>
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取电池" disabled={busy} onClick={reread} /></div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <div className="ccc-display-board ccc-charge-board"><BatteryOverview profile={profile} />
    {!profile ? <p className="ccc-control-caption">{error ? "暂无有效电池读数，请重新读取。" : "正在读取电池。"}</p> : !charge || !draft ?
      <p className="ccc-control-caption">暂未读取到可用的充电保护接口。</p> : <>
      <div className="ccc-display-controls">
      <div className="ccc-keyboard-setting">
        <strong>限制充电范围</strong>
        <Switch label="限制充电范围" checked={draft.enabled} disabled={busy || !canWrite} onChange={enabled => update({ enabled })} />
      </div>
      <div className="ccc-battery-thresholds">
        <div><span className="ccc-control-caption">开始充电</span><Select label="开始充电阈值" value={String(draft.start)} options={choices(charge.startValues, draft.start)} disabled={busy || !canWrite || !draft.enabled}
          onChange={value => update({ start: Number(value) })} /></div>
        <div><span className="ccc-control-caption">停止充电</span><Select label="停止充电阈值" value={String(draft.stop)} options={choices(charge.stopValues, draft.stop)} disabled={busy || !canWrite || !draft.enabled}
          onChange={value => update({ stop: Number(value) })} /></div>
      </div>
      <p className="ccc-control-caption" role="status">当前 · {description(charge.current)}</p>
      {!valid && <p className="ccc-control-error">请选择可用阈值，且开始值低于停止值。</p>}
      {power?.present !== true && <p className="ccc-control-caption">未确认电池存在，请重新读取后操作。</p>}
      {profile.chargeLockReason && <p className="ccc-control-caption">{profile.chargeLockReason}</p>}
      </div>
      <ConfigurationAction revision={JSON.stringify([revision, draft])} label="充电保护" disabled={busy || !canWrite || !valid || !dirty}
        onConfirm={() => revision ? run(() => applyBatteryCharge(revision, draft), "充电设置已应用") : undefined}>
        <p>{description(charge.current)} → {description(draft)}</p>
      </ConfigurationAction>
      <ConfigurationAction revision={revision ?? ""} label="充电保护" restore disabled={busy || !canWrite || !profile.canRestore}
        onConfirm={() => revision ? run(() => restoreBatteryCharge(revision), "充电初始设置已恢复") : undefined}>
        <p>恢复首次修改前的充电保护范围。</p>
      </ConfigurationAction>
    </>}
    </div>
  </Surface>
    <div className="ccc-device-pair">
      <EnergyPolicySettings active={active} revision={refreshRevision} locked={chargeBusy || calibrationBusy || calibrationLocked || power?.present !== true} onChanged={refreshFacts} onBusy={setEnergyBusy} />
      <BatteryCalibrationSettings active={active} locked={chargeBusy || energyBusy} onChanged={refreshFacts} onBusy={setCalibrationBusy} onLocked={setCalibrationLocked} />
    </div>
  </div>;
}
