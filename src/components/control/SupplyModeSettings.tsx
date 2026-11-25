import { OEM_POWER_MODES } from "../../api/client";
import { Icon } from "../Icon";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { ConfirmButton } from "../ConfirmButton";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { SupplyModeRecovery } from "./SupplyModeRecovery";
import { useSupplyMode } from "./useSupplyMode";

export function SupplyModeSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const p = useSupplyMode(active, revision, onSettled), { view } = p;
  if (view && !view.review && !view.recovery && !view.canRestore && !view.blockedReason && !view.error && !p.error) return null;
  const status = view?.sleeping ? "等待唤醒" : view?.enabled && !view.active ? "已暂停" : !view && !p.error ? "读取中…" : null;
  const issues = Array.from(new Set([view?.blockedReason, p.error, view?.error, view?.issue, view?.recoveryError].filter(Boolean)));
  const mutating = p.busy || p.pausing;
  const review = view?.review;
  const archiveRevision = view?.archiveRevision;
  const canStart = !!review && !!view && (view.canEnable || view.canResume);
  const toggle = (enabled: boolean) => {
    if (!view || mutating) return;
    if (!enabled && view.active) void p.pause();
    if (enabled && canStart && !view.active) void (view.canResume ? p.resume() : p.enable());
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="原厂供电模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />原厂供电模式</h2>
      <div className="ccc-module-tools">
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取供电模式" disabled={mutating} onClick={() => void p.reread()} />
      </div>
    </div>
    {review && <p className="ccc-control-caption">当前模式 · {OEM_POWER_MODES[review.currentMode] ?? "未知"}</p>}
    <div className="ccc-keyboard-setting">
      <div><strong>按供电自动切换</strong>{status && <p className="ccc-control-caption" role="status">{status}</p>}</div>
      <Switch label="启用原厂自动切换" disabled={mutating || !view?.active && !canStart} checked={!!view?.active} onChange={toggle} />
    </div>
    {view && !view.listenerOnline && <p className="ccc-control-caption">供电监听未就绪</p>}
    {issues.map(issue => <p key={issue} className="ccc-control-error" role="alert">{issue}</p>)}
    <OemConflictAction issues={issues} disabled={mutating} onClosed={p.reread} />
    {archiveRevision && <div className="ccc-actions">
      <ConfirmButton label="归档无效供电记录" confirmLabel="确认归档" revision={archiveRevision}
        busy={mutating} onConfirm={() => p.archive(archiveRevision)}>
        <p>确认后暂停自动切换，并把原文件移到同目录下的独立归档文件夹。当前设备设置将成为下次启用时的基线。</p>
      </ConfirmButton>
    </div>}
    {(p.busy || !view && p.error) && <div className="ccc-actions ccc-module-control-end">
      <Button icon="pause" variant="secondary" size="compact" disabled={p.pausing || (p.busy && p.pauseRequested)}
        aria-busy={p.pausing} onClick={() => void p.pause()}>{p.pausing ? "正在暂停…" : "暂停自动切换"}</Button>
    </div>}
    {p.busy && p.pauseRequested && <p className="ccc-control-caption" role="status">已暂停后续操作，等待当前操作结束。</p>}
    {view && (
      <ConfigurationAction label="原厂供电模式" revision={view.revision} restore disabled={mutating || !view.canRestore}
        onConfirm={p.restore}>
        <p>停止自动切换，并恢复下列已接管的设置。</p>
        {view.recovery ? <SupplyModeRecovery value={view.recovery} /> : <p>尚无设备设置需要恢复。</p>}
      </ConfigurationAction>
    )}
  </Surface>;
}
