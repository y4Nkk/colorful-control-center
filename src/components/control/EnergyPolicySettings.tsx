import { Icon } from "../Icon";
import { applyEnergyPolicy, restoreEnergyPolicy } from "../../api/client";
import { energySummary, energyError } from "../../lib/energy-policy";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { EnergyPolicyEditor } from "./EnergyPolicyEditor";
import { useEnergyPolicy } from "./useEnergyPolicy";

export function EnergyPolicySettings({ active, revision, locked, onChanged, onBusy }: { active: boolean; revision: number; locked: boolean; onChanged: () => void; onBusy: (busy: boolean) => void }) {
  const model = useEnergyPolicy(active, revision, onChanged, onBusy);
  const { profile, draft } = model, busy = locked || model.busy;
  const dirty = !!profile && !!draft && JSON.stringify(profile.saved) !== JSON.stringify(draft);
  const invalid = profile && draft ? energyError(draft, profile) : null;
  const disabled = busy || !profile?.canWrite || !!profile.recoveryRequired;
  if (model.loaded && !profile && !model.error) return null;
  return <Surface as="section" variant="inset" className="ccc-energy-policy" aria-label="电池节能计划">
    <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="sleep" />节能计划</h3><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取节能计划</Button></div>
    {(model.error || profile?.error) && <p className="ccc-control-error" role="alert">{model.error || profile?.error}</p>}
    {!profile || !draft ? <p className="ccc-control-caption">读取原厂保存计划后可预览和设置。</p> : <>
      <div className="ccc-keyboard-setting"><strong>按时段使用电池</strong>
        <Switch label="启用节能计划" checked={draft.enabled} disabled={disabled} onChange={enabled => model.setDraft({ ...draft, enabled })} /></div>
      <EnergyPolicyEditor draft={draft} profile={profile} disabled={disabled} onChange={model.setDraft} />
      <div className="ccc-energy-preview" role="status" aria-label="节能计划预览"><p>保存计划：{profile.saved ? energySummary(profile.saved) : "未保存计划"}</p>
        {dirty && <p>待应用：{energySummary(draft)}</p>}
        <p>充电回读：{profile.chargeReported.enabled ? `${profile.chargeReported.start}–${profile.chargeReported.stop}%` : "充至满电"}</p>
        {!draft.enabled && <p>停止后充电：{profile.chargeOnStop.current.enabled ? `${profile.chargeOnStop.current.start}–${profile.chargeOnStop.current.stop}%` : "充至满电"}</p>}
      </div>
      {invalid && <p className="ccc-control-error">{invalid}</p>}
      {profile.recoveryRequired && <p className="ccc-control-error">上次操作尚需恢复，请先恢复节能初始。</p>}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify([profile.revision, draft])} label={dirty ? "应用节能计划" : "重新发送保存计划"} confirmLabel="确认发送并保存节能计划？" busy={busy} disabled={disabled || !!invalid}
          onConfirm={() => model.run(() => applyEnergyPolicy(profile.revision, draft))} />
        {profile.saved?.enabled && <ConfirmButton icon="pause" variant="secondary" label="停止节能" confirmLabel="停止节能时段并恢复原充电策略？" busy={busy} disabled={disabled}
          onConfirm={() => model.run(async () => {
            await applyEnergyPolicy(profile.revision, { ...profile.saved!, enabled: false });
            model.setDraft(previous => previous ? { ...previous, enabled: false } : previous);
          }, { preserveDraft: true })} />}
        <ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复节能初始" confirmLabel={profile.hasInitialPlan ? "恢复首次接管前的节能与充电策略？" : "清除本应用时段并恢复初始充电策略？"} busy={busy} disabled={busy || !profile.canWrite || !profile.canRestore}
          onConfirm={() => model.run(() => restoreEnergyPolicy(profile.revision))} />
      </div>
      <Collapsible summary="计划来源与恢复目标"><FactList compact layout="tiles" facts={[
        { label: "计划来源", value: profile.source === "oemSaved" ? "原厂保存" : profile.source === "unconfigured" ? "未保存" : "本应用保存" },
        { label: "实际时段执行", value: "未回读" }, { label: "初始计划", value: profile.hasInitialPlan ? "还原保存计划" : "清除本应用时段" },
        { label: "初始充电", value: profile.initialCharge.enabled ? `${profile.initialCharge.start}–${profile.initialCharge.stop}%` : "充至满电" },
      ]} /><p>计划由固件执行，停止时恢复原充电策略。</p></Collapsible>
    </>}
  </Surface>;
}
