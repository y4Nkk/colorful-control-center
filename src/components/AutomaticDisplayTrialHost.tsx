import { useEffect, useRef } from "react";
import { onAutomaticDisplayTrial } from "../api/client";
import { refreshAutomaticDisplayTrial, resolveAutomaticDisplayTrial, useAutomaticDisplayTrial } from "../app/automatic-display-trial";
import { Button } from "./Button";
import { OemConflictAction } from "./OemConflictAction";
import { useOemConflict } from "./OemConflictHost";
import { Surface } from "./Surface";

export function AutomaticDisplayTrialHost() {
  const { pending, busy, error, readError } = useAutomaticDisplayTrial();
  const conflict = useOemConflict();
  const region = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let disposed = false, polling = false, timer: ReturnType<typeof setTimeout> | undefined;
    let unlisten: (() => void) | undefined;
    const poll = async () => {
      if (polling || disposed) return;
      polling = true; const hasTrial = await refreshAutomaticDisplayTrial(); polling = false;
      if (!disposed) timer = setTimeout(poll, hasTrial ? 500 : 2000);
    };
    void onAutomaticDisplayTrial(() => { if (timer) clearTimeout(timer); void poll(); })
      .then(stop => { if (disposed) stop(); else { unlisten = stop; void poll(); } })
      .catch(() => { void poll(); });
    return () => { disposed = true; if (timer) clearTimeout(timer); unlisten?.(); };
  }, []);
  useEffect(() => { if (pending?.trialId != null) region.current?.focus(); }, [pending?.trialId]);
  if (!pending) return null;
  const before = pending.before[0], target = pending.targets[0];
  const seconds = pending.remainingMs == null ? null : Math.max(0, Math.ceil(pending.remainingMs / 1000));
  const canKeep = !busy && !readError && !pending.error && target && seconds != null && seconds > 0;
  const reread = () => void refreshAutomaticDisplayTrial(true);
  const resolve = async (keep: boolean) => conflict.review([await resolveAutomaticDisplayTrial(keep)], reread);
  return <div ref={region} className="ccc-auto-display-host" role="dialog" aria-labelledby="automatic-display-title" tabIndex={-1}>
    <Surface as="section" variant="group" className="ccc-panel">
      <div className="ccc-panel-head"><h2 id="automatic-display-title" className="ccc-panel-title">自动刷新率试用</h2>
        <span className="ccc-control-caption" role="status">{seconds == null ? "恢复待处理" : `${seconds} 秒后恢复`}</span></div>
      <p className="ccc-control-caption">确认画面正常后保留；取消或超时恢复本次切换前的模式。</p>
      <div className="ccc-auto-display-values">
        <Surface variant="inset" className="ccc-auto-refresh-facts"><span>之前</span><strong>{before ? `${before.frequency} Hz` : "未知"}</strong></Surface>
        <Surface variant="inset" className="ccc-auto-refresh-facts"><span>正在试用</span><strong>{target ? `${target.frequency} Hz` : "未知"}</strong></Surface>
      </div>
      {(error || readError || pending.error) && <p className="ccc-control-error" role="alert">{error || readError || pending.error}</p>}
      <OemConflictAction issues={[error, readError, pending.error]} disabled={busy} onClosed={reread} />
      <div className="ccc-actions">
        <Button disabled={!canKeep} onClick={() => { void resolve(true); }}>保留刷新率</Button>
        <Button icon="undo" variant="secondary" disabled={busy || pending.trialId == null} onClick={() => { void resolve(false); }}>恢复本次试用</Button>
      </div>
    </Surface>
  </div>;
}
