import { Icon } from "../Icon";
import { applyEnergyPolicy, ENERGY_POLICY_LABEL, restoreEnergyPolicy } from "../../api/client";
import { energySummary, energyError } from "../../lib/energy-policy";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { EnergyPolicyEditor } from "./EnergyPolicyEditor";
import { EnergyTimeline } from "./EnergyTimeline";
import { useEnergyPolicy } from "./useEnergyPolicy";

export function EnergyPolicySettings({ active, revision, locked, onChanged, onBusy }: { active: boolean; revision: number; locked: boolean; onChanged: () => void; onBusy: (busy: boolean) => void }) {
  const model = useEnergyPolicy(active, revision, onChanged, onBusy);
  const { profile, current, draft, dirty } = model, busy = locked || model.busy;
  const invalid = profile && draft ? energyError(draft, profile) : null;
  const disabled = busy || !profile?.canWrite || !!profile.recoveryRequired;
  const activationIssue = profile && current && !current.enabled ? energyError({ ...current, enabled: true }, profile) : null;
  const toggle = (enabled: boolean) => {
    if (!profile || !current || disabled || current.enabled === enabled || enabled && activationIssue) return;
    void model.run(() => applyEnergyPolicy(profile.revision, { ...current, enabled }), { preserveDraft: true });
  };
  if (model.loaded && !profile && !model.error) return null;
  return <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="energy-policy-heading">
    <div className="ccc-panel-head"><h2 id="energy-policy-heading" className="ccc-panel-title"><Icon name="sleep" />{ENERGY_POLICY_LABEL}</h2>
      <div className="ccc-module-tools"><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取节能计划" disabled={busy} onClick={model.reread} /></div>
    </div>
    {(model.error || profile?.error) && <p className="ccc-control-error" role="alert">{model.error || profile?.error}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">读取原厂保存计划后可预览和设置。</p> : <>
      <div className="ccc-keyboard-setting"><strong>按时段使用电池</strong>
        <Switch label="启用节能计划" checked={current?.enabled ?? false} disabled={disabled || !!activationIssue} onChange={toggle} /></div>
      {activationIssue && <p className="ccc-control-caption">{activationIssue}</p>}
      {(dirty || profile.saved) && <EnergyTimeline slots={draft.slots} label={dirty ? "待应用时段" : "已保存时段"} />}
      {invalid && <p className="ccc-control-error">{invalid}</p>}
      {profile.recoveryRequired && <p className="ccc-control-error">上次操作尚需恢复，请先恢复节能初始。</p>}
      <div className="ccc-actions ccc-module-control-end">
        <DetailsDialog title="编辑节能时段" icon="edit" editor wide>
          <EnergyPolicyEditor draft={draft} profile={profile} disabled={disabled} onChange={model.setDraft} onAutomaticChange={model.changeAutomatic} />
          {invalid && <p className="ccc-control-error"><Icon name="error" size={16} />{invalid}</p>}
          <ConfigurationAction revision={JSON.stringify([profile.revision, draft])} label={ENERGY_POLICY_LABEL}
            applyLabel="保存节能时段" disabled={disabled || !!invalid || !dirty}
            onConfirm={() => model.run(() => applyEnergyPolicy(profile.revision, draft))} />
        </DetailsDialog>
        <ConfigurationAction revision={profile.revision} label={ENERGY_POLICY_LABEL} restore disabled={busy || !profile.canWrite || !profile.canRestore}
          onConfirm={() => model.run(() => restoreEnergyPolicy(profile.revision))}>
          <p>{profile.hasInitialPlan ? "还原首次接管前的节能计划。" : "清除本应用设置的节能时段。"}</p>
          <p>恢复初始充电 · {profile.initialCharge.enabled ? `${profile.initialCharge.start}–${profile.initialCharge.stop}%` : "充至满电"}</p>
        </ConfigurationAction>
      <DetailsDialog title="计划来源与恢复目标" label="详情"><FactList compact layout="tiles" facts={[
        { label: "计划来源", value: profile.source === "oemSaved" ? "原厂保存" : profile.source === "unconfigured" ? "未保存" : "本应用保存" },
        { label: "实际时段执行", value: "未回读" }, { label: "初始计划", value: profile.hasInitialPlan ? "还原保存计划" : "清除本应用时段" },
        { label: "初始充电", value: profile.initialCharge.enabled ? `${profile.initialCharge.start}–${profile.initialCharge.stop}%` : "充至满电" },
      ]} /><p>计划由固件执行，停止时恢复原充电策略。</p>
        {profile.saved && <><p>{energySummary(profile.saved)}</p>
          <Button icon="refresh" variant="secondary" disabled={disabled}
            onClick={() => void model.run(() => applyEnergyPolicy(profile.revision, profile.saved!), { preserveDraft: true })}>重新发送保存计划</Button>
        </>}
      </DetailsDialog>
      </div>
    </>}
  </Surface>;
}
