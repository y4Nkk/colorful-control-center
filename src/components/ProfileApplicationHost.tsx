import { Icon } from "./Icon";
import { useEffect } from "react";
import { cancelProfileApplication, findOemConflict, keepProfileDisplay, type ProfileApplication } from "../api/client";
import { refreshProfileApplication, runProfileApplication, useProfileApplicationState } from "../app/profile-application";
import { Button } from "./Button";
import { OemConflictAction } from "./OemConflictAction";
import { useOemConflict } from "./OemConflictHost";
import { Surface } from "./Surface";
import { toast } from "./use-toast";

const failedSteps = (report: ProfileApplication | null) => report?.steps.filter(step => !step.ok).map(step => step.message) ?? [];

export function ProfileApplicationHost() {
  const { report, busy, error } = useProfileApplicationState();
  const conflict = useOemConflict();
  useEffect(() => {
    let disposed = false, timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await refreshProfileApplication();
      if (!disposed) timer = setTimeout(poll, 500);
    };
    void poll();
    return () => { disposed = true; clearTimeout(timer); };
  }, []);
  const pending = report?.pending;
  if (!pending && error?.kind !== "read" && !findOemConflict([error?.message])) return null;
  const seconds = pending?.remainingMs == null ? null : Math.max(0, Math.ceil(pending.remainingMs / 1000));
  const canKeep = Boolean(pending && !pending.error && seconds != null && seconds > 0 && report?.steps.every(step => step.ok));
  const problem = error?.message ?? pending?.error;
  const reread = () => void refreshProfileApplication(true);
  const run = async (keep: boolean) => {
    if (!report) return;
    try {
      const result = await runProfileApplication(() => keep ? keepProfileDisplay(report.id) : cancelProfileApplication(report.id));
      conflict.review([result.pending?.error, ...failedSteps(result)], reread);
      if (!result.pending) toast({ kind: result.targetsApplied && result.steps.every(step => step.ok) ? "success" : "warning",
        title: result.targetsApplied ? result.steps.every(step => step.ok) ? `${result.name} 已应用` : "目标设置已应用，部分收尾待处理" : "档案应用已结束，请查看执行结果" });
    } catch (error) {
      const message = String(error);
      conflict.review([message], reread);
      toast({ kind: "error", title: "档案确认或恢复失败", description: message });
    }
  };
  return <Surface as="section" variant="group" className="ccc-panel ccc-profile-trial" aria-label="档案显示确认">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title">{report?.name ?? "档案执行状态"}</h2>
      {pending && <span className="ccc-control-caption">{busy ? "正在处理档案" : pending.error || seconds == null ? "显示恢复待处理" : `显示试用 · ${seconds} 秒`}</span>}</div>
    {pending && <>
      <p className="ccc-control-caption">{busy ? "正在重新核对并处理档案，请等待执行结果。" : pending.error ? "显示恢复尚未完成，请查看执行结果并重试恢复。" : seconds == null ? "显示已保留；请查看执行结果，恢复失败可重试。" : "确认所有屏幕显示正常后保留，再继续应用其余档案设置；超时恢复本次显示前态。"}</p>
      <div className="ccc-profile-trial-displays">{pending.targets.map(target => <Surface variant="inset" className="ccc-profile-device" key={target.device}>
        <div><strong>{target.name}</strong><p className="ccc-control-caption">{target.width} × {target.height}</p></div>
        <output className="ccc-mono">{target.frequency} Hz</output>
      </Surface>)}</div>
      <div className="ccc-actions"><Button disabled={busy || !canKeep} onClick={() => { void run(true); }}>保留显示并继续档案</Button>
        <Button icon="undo" variant="secondary" disabled={busy} onClick={() => { void run(false); }}>恢复本次显示并停止</Button></div>
    </>}
    {problem && <p className="ccc-control-error" role="alert">{problem}</p>}
    <OemConflictAction issues={[problem, ...failedSteps(report)]} disabled={busy} onClosed={reread} />
    {report?.steps.some(step => !step.ok) && <ul className="ccc-profile-steps" aria-label="档案显示执行结果">{report.steps.map((step, index) => <li key={index}><span className="ccc-step-status"><Icon name={step.ok ? "check-circle" : "error"} label={step.ok ? "成功" : "失败"} size={16} />{step.step}</span><span>{step.message}</span></li>)}</ul>}
    {!pending && error && <Button icon="refresh" variant="secondary" disabled={busy} onClick={() => { void refreshProfileApplication(true); }}>重新读取档案状态</Button>}
  </Surface>;
}
