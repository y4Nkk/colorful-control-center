import { useState } from "react";
import { OEM_POWER_MODES } from "../../api/client";
import { Icon } from "../Icon";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { PowerSourceVisual } from "./PowerSourceVisual";
import { SupplyModeCurrent } from "./SupplyModeCurrent";
import { SupplyModeRecovery } from "./SupplyModeRecovery";
import { SupplyModeScenarios } from "./SupplyModeScenarios";
import { useSupplyMode } from "./useSupplyMode";

export function SupplyModeSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const p = useSupplyMode(active, revision, onSettled), { view } = p;
  const [staged, setStaged] = useState(false);
  if (view && !view.review && !view.recovery && !view.canRestore && !view.error && !p.error) return null;
  const status = view?.active ? "已启用" : view?.sleeping ? "等待唤醒" : view?.enabled ? "已暂停" : "未启用";
  const issues = Array.from(new Set([p.error, view?.error, view?.issue, view?.recoveryError].filter(Boolean)));
  const mutating = p.busy || p.pausing;
  const review = view?.review;
  const canStart = !!view && (view.canEnable || view.canResume);
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="原厂供电模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />原厂供电模式</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="供电状态与切换规则" label="详情" wide>
          <div className="ccc-detail-stack">
            {view && <SupplyModeCurrent view={view} />}
            {review && <SupplyModeScenarios review={review} />}
            {issues.map(issue => <p key={issue} className="ccc-control-error">{issue}</p>)}
            {view?.recovery && !view.canRestore && <DetailsDialog title="供电恢复详情" wide>
              <SupplyModeRecovery value={view.recovery} />
            </DetailsDialog>}
          </div>
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取供电模式" disabled={mutating} onClick={() => void p.reread()} />
      </div>
    </div>
    <PowerSourceVisual source={review?.onAc === true ? "ac" : review?.onAc === false ? "battery" : "unknown"}
      label={review ? OEM_POWER_MODES[review.currentMode] ?? "模式未知" : "当前供电"} />
    <div className="ccc-keyboard-setting">
      <div><strong>按供电自动切换</strong><p className="ccc-control-caption">{view ? status : p.error ? "读取失败" : "读取中…"}</p></div>
      <Switch label="启用原厂自动切换" disabled={mutating || !canStart || !!view?.active} checked={staged || !!view?.active} onChange={setStaged} />
    </div>
    {view && !view.listenerOnline && <p className="ccc-control-caption">供电监听未就绪</p>}
    {issues.length > 0 && <p role="status" className="ccc-control-caption">部分状态需要处理 · 查看详情</p>}
    {(view?.active || p.busy || p.error) && <div className="ccc-actions ccc-module-control-end">
      <Button icon="pause" variant="secondary" size="compact" disabled={p.pausing || (p.busy && p.pauseRequested)}
        aria-busy={p.pausing} onClick={() => void p.pause()}>{p.pausing ? "正在暂停…" : "暂停自动切换"}</Button>
    </div>}
    {p.busy && p.pauseRequested && <p className="ccc-control-caption" role="status">已暂停后续操作，等待当前操作结束。</p>}
    {view && <>
      <ConfigurationAction label="原厂供电模式" revision={view.revision} disabled={mutating || !staged || !canStart || !review}
        onConfirm={async () => { if (await (view.canResume ? p.resume() : p.enable())) setStaged(false); }}>
        <p>{view.canResume ? "继续自动切换" : "启用自动切换"}并应用当前启动规则。</p>
        {review && <SupplyModeScenarios review={review} />}
      </ConfigurationAction>
      <ConfigurationAction label="原厂供电模式" revision={view.revision} restore disabled={mutating || !view.canRestore}
        onConfirm={async () => { if (await p.restore()) setStaged(false); }}>
        <p>停止自动切换，并恢复下列已接管的设置。</p>
        {view.recovery ? <SupplyModeRecovery value={view.recovery} /> : <p>尚无设备设置需要恢复。</p>}
      </ConfigurationAction>
    </>}
  </Surface>;
}
