import { Icon } from "../Icon";
import { useEffect } from "react";
import { ENERGY_POLICY_LABEL, restoreBatteryCalibration, startBatteryCalibration, stopBatteryCalibration, type CalibrationPhase, type CalibrationStatus } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { useBatteryCalibration } from "./useBatteryCalibration";

const phases: { id: CalibrationPhase; label: string }[] = [
  { id: "discharge1", label: "第一次放电" }, { id: "charge1", label: "第一次充电" },
  { id: "discharge2", label: "第二次放电" }, { id: "charge2", label: "第二次充电" }, { id: "adjusting", label: "调整放电" },
];
const statusNames: Record<CalibrationStatus, string> = { running: "正在校准", paused: "已暂停，等待接入电源", completed: "校准完成，已恢复策略",
  stopped: "已停止，已恢复策略", failed: "校准已中止", recoveryRequired: "控制尚需恢复" };
const elapsed = (ms: number) => {
  const seconds = Math.floor(ms / 1000);
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(v => String(v).padStart(2, "0")).join(":");
};
interface Props { active: boolean; locked: boolean; onChanged: () => void; onBusy: (busy: boolean) => void; onLocked: (locked: boolean) => void }
export function BatteryCalibrationSettings({ active, locked, onChanged, onBusy, onLocked }: Props) {
  const model = useBatteryCalibration(active, onChanged, onBusy);
  const { profile, error, loaded, busy } = model;
  const ownsCharge = !!profile && (profile.workerRunning || profile.recoveryRequired);
  useEffect(() => onLocked(ownsCharge), [ownsCharge, onLocked]);
  if (loaded && !profile && !error) return null;
  const task = profile?.task;
  const taskPercent = model.stale ? null : task?.facts.percent;
  const disabled = busy || locked || model.stale;
  const revision = profile?.revision;
  const status = profile?.recoveryRequired ? profile.controlAcquired ? "上次校准尚需恢复" : "需要解除准备记录"
    : task ? statusNames[task.status] : profile?.workerRunning ? "正在准备" : null;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="电池校准">
    <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="battery" />电池校准</h3>
      <div className="ccc-module-tools"><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取校准"
        disabled={busy || locked} onClick={model.reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <OemConflictAction issues={[error, profile?.unavailableReason, profile?.error]} disabled={busy || locked} onClosed={model.reread} />
    {profile ? <>
      {status && <div className="ccc-module-summary">
        <strong role="status">{status}</strong>
        {task && <span className="ccc-control-caption">{phases.find(phase => phase.id === task.phase)?.label} · {elapsed(task.elapsedMs)}</span>}
      </div>}
      {profile.unavailableReason && <p className="ccc-control-caption">{profile.unavailableReason}</p>}
      {profile.error && <p className="ccc-control-error" role="alert">{profile.error}</p>}
      <div className="ccc-actions ccc-module-control-end">
        {!profile.workerRunning && <ConfirmButton icon="play" key={`start-${revision}`} label="开始电池校准" confirmLabel="开始校准" busy={busy} disabled={disabled || !profile.canStart}
          onConfirm={() => revision ? model.run(() => startBatteryCalibration(revision)) : undefined}>
          <p>两轮充放电，最后放电至 {profile.stopThreshold}%。校准时屏幕常亮，暂停睡眠、休眠和充电保护。</p>
        </ConfirmButton>}
        {profile.canStop && <ConfirmButton icon="pause" key={`stop-${revision}`} label="停止校准" confirmLabel="停止并恢复" busy={busy} disabled={disabled}
          onConfirm={() => revision ? model.run(() => stopBatteryCalibration(revision)) : undefined}><p>停止校准并恢复原充电策略。</p></ConfirmButton>}
        <DetailsDialog title="校准阶段与恢复" label="详情">
          <ol aria-label="校准阶段">{phases.map(phase => <li key={phase.id} aria-current={task?.phase === phase.id ? "step" : undefined}>{phase.label}</li>)}</ol>
          {task && <p className="ccc-control-caption">本阶段已用 {elapsed(task.phaseElapsedMs)}</p>}
          {profile.workerRunning && task && <p className="ccc-control-caption">任务采样电量 · {taskPercent != null && taskPercent >= 0 && taskPercent <= 100 ? `${taskPercent}%` : "—"}</p>}
          <CalibrationRestoreFacts profile={profile} />
        </DetailsDialog>
      </div>
      <ConfigurationAction revision={revision ?? ""} label="电池校准" restore
        disabled={disabled || !profile.recoveryRequired || profile.workerRunning}
        onConfirm={() => revision ? model.run(() => restoreBatteryCalibration(revision)) : undefined}>
        {profile.controlAcquired ? <CalibrationRestoreFacts profile={profile} /> : <p>解除未接管的校准准备记录。</p>}
      </ConfigurationAction>
    </> : <p className="ccc-control-caption">正在读取校准能力与恢复记录。</p>}
  </Surface>;
}

function CalibrationRestoreFacts({ profile }: { profile: NonNullable<ReturnType<typeof useBatteryCalibration>["profile"]> }) {
  return <FactList compact facts={[
    { label: "Windows 电源计划", value: "恢复开始前计划" },
    { label: "充电保护", value: profile.restoreCharge.enabled ? `${profile.restoreCharge.start}–${profile.restoreCharge.stop}%` : "充至满电" },
    ...(profile.hasEnergyCapability ? [{ label: ENERGY_POLICY_LABEL, value: profile.knownEnergyPlan ? profile.restoreEnergyEnabled ? "恢复启用" : "恢复停用" : "清除本应用时段" }] : []),
  ]} />;
}
