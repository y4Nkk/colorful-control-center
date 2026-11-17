import { Icon } from "../Icon";
import { useEffect } from "react";
import { restoreBatteryCalibration, startBatteryCalibration, stopBatteryCalibration, type CalibrationPhase, type CalibrationStatus } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { useBatteryCalibration } from "./useBatteryCalibration";
import { BatteryLevel } from "./BatteryOverview";

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
  const disabled = busy || locked || model.stale;
  const revision = profile?.revision;
  return <Surface as="section" variant="inset" className="ccc-energy-policy" aria-label="电池校准">
    <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="battery" />电池校准</h3>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy || locked} onClick={model.reread}>重新读取校准</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile ? <>
      <div className="ccc-calibration-summary">
        <div><strong role="status">{profile.recoveryRequired ? profile.controlAcquired ? "上次校准尚需恢复" : "校准未接管，需要解除准备记录" : task ? statusNames[task.status] : profile.workerRunning ? "正在接管校准任务" : "尚未开始校准"}</strong>
          <p className="ccc-control-caption">两轮充放电 → {profile.stopThreshold}%</p>
          <span className="ccc-control-caption">校准期间设置</span>
          <FactList compact layout="tiles" facts={[{ label: "屏幕亮度", value: "100% · 常亮" },
            { label: "自动睡眠 / 休眠", value: "暂停" }, { label: "充电保护 / 节能计划", value: "暂停" }]} />
          {task?.status === "running" && task.phase.startsWith("discharge") && <span className="ccc-chip">电池放电中</span>}</div>
        <div className="ccc-calibration-live"><BatteryLevel label="校准当前电量" percent={model.stale ? null : task?.facts.percent ?? null} charging={null} />
          <FactList compact facts={[{ label: "已用时间", value: task ? elapsed(task.elapsedMs) : "—" },
            { label: "本阶段", value: task ? elapsed(task.phaseElapsedMs) : "—" }]} /></div>
      </div>
      <ol className="ccc-calibration-steps" aria-label="校准阶段">{phases.map((phase, i) => <li key={phase.id} aria-current={task?.phase === phase.id ? "step" : undefined}>
        <Surface variant="card" className="ccc-calibration-stage"><span className="ccc-rule-heading"><Icon name={phase.id.startsWith("charge") ? "charging" : "battery"} /><span className="ccc-control-caption">{i + 1}</span></span><strong>{phase.label}</strong></Surface>
      </li>)}</ol>
      <Collapsible summary="校准恢复目标" aria-label="校准恢复目标">
        <FactList compact layout="tiles" facts={[{ label: "Windows 电源计划", value: "恢复开始前计划" },
          { label: "充电保护", value: profile.restoreCharge.enabled ? `${profile.restoreCharge.start}–${profile.restoreCharge.stop}%` : "充至满电" },
          ...(profile.hasEnergyCapability ? [{ label: "节能计划", value: profile.knownEnergyPlan ? profile.restoreEnergyEnabled ? "恢复启用" : "恢复停用" : "未读取 · 清除本应用时段" }] : [])]} />
        <p>停止、完成或异常后释放校准控制，移除临时电源计划并恢复上述策略。校准在后台运行，切换页面不影响任务。</p>
      </Collapsible>
      {profile.unavailableReason && <p className="ccc-control-caption">{profile.unavailableReason}</p>}
      {profile.error && <p className="ccc-control-error" role="alert">{profile.error}</p>}
      <div className="ccc-actions">
        <ConfirmButton icon="play" key={`start-${revision}`} label="开始电池校准" confirmLabel="确认开始完整校准？" busy={busy} disabled={disabled || !profile.canStart}
          onConfirm={() => revision ? model.run(() => startBatteryCalibration(revision)) : undefined} />
        <ConfirmButton icon="pause" key={`stop-${revision}`} variant="secondary" label="停止校准" confirmLabel="停止校准并恢复原策略？" busy={busy} disabled={disabled || !profile.canStop}
          onConfirm={() => revision ? model.run(() => stopBatteryCalibration(revision)) : undefined} />
        <ConfirmButton key={`restore-${revision}`} variant="secondary" label={profile.controlAcquired ? "重试校准恢复" : "解除准备记录"} confirmLabel={profile.controlAcquired ? "释放校准控制并恢复捕获策略？" : "解除未接管的校准准备记录？"} busy={busy} disabled={disabled || !profile.recoveryRequired || profile.workerRunning}
          onConfirm={() => revision ? model.run(() => restoreBatteryCalibration(revision)) : undefined} />
      </div>
    </> : <p className="ccc-control-caption">正在读取校准能力与恢复记录。</p>}
  </Surface>;
}
