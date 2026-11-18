import { Icon } from "../Icon";
import { applyEnergyPolicy, restoreEnergyPolicy } from "../../api/client";
import { energySummary, energyError } from "../../lib/energy-policy";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
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
  const { profile, draft, dirty } = model, busy = locked || model.busy;
  const invalid = profile && draft ? energyError(draft, profile) : null;
  const disabled = busy || !profile?.canWrite || !!profile.recoveryRequired;
  if (model.loaded && !profile && !model.error) return null;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="电池节能计划">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="sleep" />节能计划</h2>
      <div className="ccc-module-tools"><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取节能计划" disabled={busy} onClick={model.reread} /></div>
    </div>
    {(model.error || profile?.error) && <p className="ccc-control-error" role="alert">{model.error || profile?.error}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">读取原厂保存计划后可预览和设置。</p> : <>
      <div className="ccc-keyboard-setting"><strong>按时段使用电池</strong>
        <Switch label="启用节能计划" checked={draft.enabled} disabled={disabled} onChange={enabled => model.setDraft({ ...draft, enabled })} /></div>
      {(dirty || profile.saved) && <EnergyTimeline slots={draft.slots} label={dirty ? "待应用时段" : "已保存时段"} />}
      {invalid && <p className="ccc-control-error">{invalid}</p>}
      {profile.recoveryRequired && <p className="ccc-control-error">上次操作尚需恢复，请先恢复节能初始。</p>}
      <div className="ccc-actions ccc-module-control-end">
        <DetailsDialog title="编辑节能时段" icon="edit" editor wide>
          <EnergyPolicyEditor draft={draft} profile={profile} disabled={disabled} onChange={model.setDraft} onAutomaticChange={model.changeAutomatic} />
          {invalid && <p className="ccc-control-error">{invalid}</p>}
        </DetailsDialog>
        <ConfigurationAction revision={JSON.stringify([profile.revision, draft])} label="节能计划" disabled={disabled || !!invalid || !dirty}
          onConfirm={() => model.run(() => applyEnergyPolicy(profile.revision, draft))}>
          <p>{profile.saved ? energySummary(profile.saved) : "未保存计划"} → {energySummary(draft)}</p>
          {!draft.enabled && <p>停止后充电 · {profile.chargeOnStop.current.enabled ? `${profile.chargeOnStop.current.start}–${profile.chargeOnStop.current.stop}%` : "充至满电"}</p>}
        </ConfigurationAction>
        {profile.saved?.enabled && <ConfirmButton icon="pause" label="停止节能" confirmLabel="停止节能并恢复充电策略" busy={busy} disabled={disabled}
          onConfirm={() => model.run(async () => {
            await applyEnergyPolicy(profile.revision, { ...profile.saved!, enabled: false });
            model.setDraft(previous => previous ? { ...previous, enabled: false } : previous);
          }, { preserveDraft: true })} />}
        <ConfigurationAction revision={profile.revision} label="节能计划" restore disabled={busy || !profile.canWrite || !profile.canRestore}
          onConfirm={() => model.run(() => restoreEnergyPolicy(profile.revision))}>
          <p>{profile.hasInitialPlan ? "还原首次接管前的节能计划。" : "清除本应用设置的节能时段。"}</p>
          <p>恢复初始充电 · {profile.initialCharge.enabled ? `${profile.initialCharge.start}–${profile.initialCharge.stop}%` : "充至满电"}</p>
        </ConfigurationAction>
      <DetailsDialog title="计划来源与恢复目标" label="详情"><FactList compact layout="tiles" facts={[
        { label: "计划来源", value: profile.source === "oemSaved" ? "原厂保存" : profile.source === "unconfigured" ? "未保存" : "本应用保存" },
        { label: "实际时段执行", value: "未回读" }, { label: "初始计划", value: profile.hasInitialPlan ? "还原保存计划" : "清除本应用时段" },
        { label: "初始充电", value: profile.initialCharge.enabled ? `${profile.initialCharge.start}–${profile.initialCharge.stop}%` : "充至满电" },
      ]} /><p>计划由固件执行，停止时恢复原充电策略。</p>
        {profile.saved && <ConfirmButton label="重新发送保存计划" confirmLabel="确认重新发送计划" busy={busy} disabled={disabled}
          onConfirm={() => model.run(() => applyEnergyPolicy(profile.revision, profile.saved!), { preserveDraft: true })} />}
      </DetailsDialog>
      </div>
    </>}
  </Surface>;
}
