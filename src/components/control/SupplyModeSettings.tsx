import { Icon } from "../Icon";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { SupplyModeCurrent } from "./SupplyModeCurrent";
import { SupplyModeRecovery } from "./SupplyModeRecovery";
import { SupplyModeScenarios } from "./SupplyModeScenarios";
import { useSupplyMode } from "./useSupplyMode";

export function SupplyModeSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const p = useSupplyMode(active, revision, onSettled), { view } = p;
  if (view && !view.review && !view.recovery && !view.canRestore && !view.error && !p.error) return null;
  const status = view?.active ? "自动切换已启用" : view?.sleeping ? "等待唤醒" : view?.enabled ? "自动切换已暂停" : "自动切换未启用";
  const issues = Array.from(new Set([p.error, view?.error, view?.issue, view?.recoveryError].filter(Boolean)));
  const mutating = p.busy || p.pausing;
  return <Surface as="section" variant="group" className="ccc-panel ccc-supply-mode" aria-label="原厂供电模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />原厂供电模式</h2><span className="ccc-chip">{status}</span></div>
    <div className="ccc-supply-mode-columns">
      <div className="ccc-supply-mode-stack">
        {view ? <SupplyModeCurrent view={view} /> : <p className="ccc-control-caption">{p.error ? "读取失败，重新读取后才能启用或恢复。" : "正在读取模式与联动目标…"}</p>}
        {view && !view.listenerOnline && <p className="ccc-control-caption">供电监听未就绪，暂不能启用自动切换。</p>}
        <div className="ccc-actions">
          <Button variant="secondary" disabled={p.pausing || (p.busy && p.pauseRequested) || (!p.busy && !view?.enabled && !view?.active && !p.error)}
            aria-busy={p.pausing} onClick={() => { void p.pause(); }}>{p.pausing ? "正在暂停…" : "暂停自动切换"}</Button>
          <Button icon="refresh" variant="secondary" disabled={mutating} onClick={() => { void p.reread(); }}>重新读取</Button>
        </div>
        {p.busy && p.pauseRequested && <p className="ccc-control-caption" role="status">已暂停后续自动操作，等待当前事务结束或恢复。</p>}
        {issues.map(issue => <p key={issue} className="ccc-control-error" role="alert">{issue}</p>)}
      </div>
      <div className="ccc-supply-mode-stack">
        {view?.review && <>
          <SupplyModeScenarios review={view.review} />
          <span className="ccc-chip is-warn">启用即应用当前启动规则</span>
          <div className="ccc-actions">
            <ConfirmButton icon="play" key={`enable:${view.revision}`} label="启用原厂自动切换" confirmLabel="按六种条件自动切换并应用启动规则？"
              disabled={!view.canEnable || mutating} busy={p.busy} onConfirm={p.enable} />
            {view.enabled && <ConfirmButton icon="play" key={`resume:${view.revision}`} label="继续自动切换" confirmLabel="继续自动切换并应用启动规则？"
              disabled={!view.canResume || mutating} busy={p.busy} onConfirm={p.resume} variant="secondary" />}
          </div>
        </>}
        {(view?.recovery || view?.canRestore) && <Collapsible summary="初始恢复预览与操作">
          <div className="ccc-supply-mode-stack">
            {view.recovery ? <SupplyModeRecovery value={view.recovery} /> : <p className="ccc-control-caption">尚未执行设备设置；本次恢复仅撤销自动策略。</p>}
            <p className="ccc-control-caption">确认后停止自动切换，并按以上初始值恢复已接管的设置。</p>
            <ConfirmButton icon="undo" key={`restore:${view.revision}`} label="恢复初始并停用" confirmLabel="恢复以上初始值并停用自动切换？"
              disabled={!view.canRestore || mutating} busy={p.busy} variant="secondary" onConfirm={p.restore} />
          </div>
        </Collapsible>}
        <Collapsible summary="自动切换规则说明"><p>按六种条件自动切换；独显可能需要系统授权，显示变化需单独保留。暂停保留已保存的启用意图，重新打开软件会继续已确认且未变化的规则；恢复初始会撤销策略。</p></Collapsible>
      </div>
    </div>
  </Surface>;
}
