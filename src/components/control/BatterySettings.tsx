import { Icon } from "../Icon";
import { applyBatteryCharge, readBatteryProfile, restoreBatteryCharge, type BatteryChargeValues } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { BatteryOverview } from "./BatteryOverview";
import { useDeviceSettings } from "./useDeviceSettings";
import { useEffect, useState } from "react";
import { EnergyPolicySettings } from "./EnergyPolicySettings";
import { BatteryCalibrationSettings } from "./BatteryCalibrationSettings";

const description = (value: BatteryChargeValues) => value.enabled ? `低于 ${value.start}% 开始，达到 ${value.stop}% 停止` : "充至满电";
const choices = (values: number[], current: number) => [...new Set([...values, current])].map(value => ({ value: String(value), label: value === 0 ? "未设置" : `${value}%` }));

export function BatterySettings({ active, revision: refreshRevision, onPolicyChanged }: { active: boolean; revision: number; onPolicyChanged: () => void }) {
  const { profile, busy: chargeBusy, error, run, reread, refresh: refreshFacts } = useDeviceSettings(readBatteryProfile, active, refreshRevision, 5000);
  const [pending, setPending] = useState<{ revision: string; values: BatteryChargeValues } | null>(null);
  const [energyBusy, setEnergyBusy] = useState(false);
  const [calibrationBusy, setCalibrationBusy] = useState(false), [calibrationLocked, setCalibrationLocked] = useState(false);
  useEffect(onPolicyChanged, [calibrationLocked, onPolicyChanged]);
  const busy = chargeBusy || energyBusy || calibrationBusy;
  const charge = profile?.charge, power = profile?.power;
  const canWrite = power?.present === true && !!charge && !!profile?.revision && !profile.chargeLockReason && !calibrationLocked;
  const revision = profile?.revision;
  const editing = !!pending && pending.revision === revision;
  const values = editing ? pending.values : charge?.current;
  const update = (patch: Partial<BatteryChargeValues>) => {
    if (!canWrite || !charge || !revision || !values || busy) return;
    const next = { ...(patch.enabled === false ? charge.current : values), ...patch };
    if (next.enabled && (!charge.startValues.includes(next.start) || !charge.stopValues.includes(next.stop) || next.start >= next.stop)) {
      setPending({ revision, values: next });
      return;
    }
    setPending(null);
    if (JSON.stringify(next) !== JSON.stringify(charge.current)) void run(() => applyBatteryCharge(revision, next), "充电设置已回读");
  };
  return <div className="ccc-module-grid">
    <Surface as="section" variant="group" className="ccc-panel" aria-label="电池与充电">
      <div className="ccc-panel-head">
        <h2 className="ccc-panel-title"><Icon name="charging" />电池与充电</h2>
        <div className="ccc-module-tools">
          <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取电池" disabled={busy} onClick={reread} />
        </div>
      </div>
      <BatteryOverview profile={profile} />
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
      {profile && charge && values ? <div className="ccc-charge-board">
        <div className="ccc-power-setting">
          <div>
            <strong>限制充电范围</strong>
            {editing && <p className="ccc-control-caption" role="status" aria-label="当前充电策略">当前 · {description(charge.current)}</p>}
          </div>
          <Switch label="限制充电范围" checked={values.enabled} disabled={busy || !canWrite} onChange={enabled => update({ enabled })} />
        </div>
        {values.enabled && <div className="ccc-power-subsettings">
          <label className="ccc-power-setting"><span>开始充电</span><span className="ccc-power-setting-picker">
            <Select label="开始充电阈值" size="compact" value={String(values.start)} options={choices(charge.startValues, values.start)}
              disabled={busy || !canWrite} onChange={value => update({ start: Number(value) })} />
          </span></label>
          <label className="ccc-power-setting"><span>停止充电</span><span className="ccc-power-setting-picker">
            <Select label="停止充电阈值" size="compact" value={String(values.stop)} options={choices(charge.stopValues, values.stop)}
              disabled={busy || !canWrite} onChange={value => update({ stop: Number(value) })} />
          </span></label>
        </div>}
        {editing && <p className="ccc-control-caption" role="status">开始值须低于停止值。阈值有效后自动生效。</p>}
        {power?.present !== true && <p className="ccc-control-caption">未确认电池存在，请重新读取后操作。</p>}
        {profile.chargeLockReason && <p className="ccc-control-caption">{profile.chargeLockReason}</p>}
        <ConfigurationAction revision={revision ?? ""} label="充电保护" restore disabled={busy || !canWrite || !profile.canRestore}
          onConfirm={() => {
            setPending(null);
            return revision ? run(() => restoreBatteryCharge(revision), "充电初始设置已恢复") : undefined;
          }}>
          <p>恢复首次修改前的充电保护范围。</p>
        </ConfigurationAction>
      </div> : profile && power?.present !== false &&
        <p className="ccc-control-caption">暂无可用的充电保护接口。</p>}
    </Surface>
    <div className="ccc-module-grid" data-module-layout="featured">
      <EnergyPolicySettings active={active} revision={refreshRevision} locked={chargeBusy || calibrationBusy || calibrationLocked || power?.present !== true} onChanged={refreshFacts} onBusy={setEnergyBusy} />
      <BatteryCalibrationSettings active={active} locked={chargeBusy || energyBusy} onChanged={refreshFacts} onBusy={setCalibrationBusy} onLocked={setCalibrationLocked} />
    </div>
  </div>;
}
