import { Icon } from "../Icon";
import { OEM_POWER_MODES } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
import { Surface } from "../Surface";
import { useAutomaticRefresh } from "./useAutomaticRefresh";

export function AutomaticRefreshSettings({ active, revision }: { active: boolean; revision: number }) {
  const model = useAutomaticRefresh(active, revision), { view, busy } = model;
  const facts = view?.current;
  const status = view?.active ? "已启用" : view?.initial ? "已暂停，保留恢复记录" : view ? "未启用" : "读取中";
  const supply = facts?.onAc === true ? "已插电" : facts?.onAc === false ? "电池供电" : "供电状态未知";
  const preview = view?.preview;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="自动刷新率">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />自动刷新率</h2><span className="ccc-chip">{status}</span></div>
    <div className="ccc-auto-refresh-columns">
      <Surface variant="inset" className="ccc-auto-refresh-facts">
        <span>当前主屏</span><strong>{facts ? `${facts.display.frequency} Hz` : "未读取"}</strong>
        <small>{facts ? `${supply} · ${OEM_POWER_MODES[facts.mode] ?? `模式 ${facts.mode}`}` : "等待设备状态"}</small>
      </Surface>
      <Surface variant="inset" className="ccc-auto-refresh-facts">
        <span>自动规则</span><strong>{facts ? `${facts.low} / ${facts.savedOrHighest} Hz` : "等待候选频率"}</strong>
        <small>{facts?.customer === 5 ? "安静或省电时降低，离开后恢复" : "电池省电或静音增强时降低，按供电与模式恢复"}</small>
      </Surface>
    </div>
    {preview && <p className="ccc-control-caption">{preview.before.frequency === preview.target.frequency ? `启用时保持 ${preview.target.frequency} Hz。` : `启用时试用 ${preview.before.frequency} → ${preview.target.frequency} Hz，需要在 15 秒内保留。`}</p>}
    {!preview && !view?.initial && facts && <span className="ccc-chip">启用时保持当前频率</span>}
    {view?.initialDisplay && <span className="ccc-chip">恢复目标 · {view.initialDisplay.frequency} Hz</span>}
    {facts && !facts.defaultEnabled && <p className="ccc-control-caption">原厂策略关闭自动刷新率，当前不能接管。</p>}
    {(!view?.listenerOnline || view?.sleeping) && view && <p className="ccc-control-caption">{view.sleeping ? "系统休眠中，自动调整已暂停。" : "供电监听未就绪，不能启用。"}</p>}
    {Array.from(new Set([model.error, view?.error, view?.issue].filter(Boolean))).map(issue => <p key={issue} className="ccc-control-error" role="alert">{issue}</p>)}
    <div className="ccc-actions">
      <ConfirmButton icon="play" key={`enable-${view?.revision}`} label="启用自动刷新率" confirmLabel="按上述规则自动试用？" disabled={!view?.canStart} busy={busy} onConfirm={model.enable} />
      <ConfirmButton icon="pause" key={`restore-${view?.revision}`} variant="secondary" label="停止并恢复" confirmLabel="停止并恢复启用前状态？" disabled={!view?.canRestore} busy={busy} onConfirm={model.restore} />
      {(model.error || view?.error || view?.issue) && <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={() => { void model.retry(); }}>重新读取</Button>}
    </div>
    <Collapsible summary="刷新率规则与恢复">
      <p>供电或模式变化时按规则试用新频率；15秒内未保留会自动恢复。停止只恢复本策略接管的设置，不覆盖外部改动。</p>
      {facts?.candidates.some(rate => rate > 255) && <p>原厂保存的频率只有一字节；超过255 Hz时恢复可能选择最高候选。</p>}
      {facts?.supplyModeSwitch && <p>供电事件还会切换性能模式，本策略等待模式变化后调整刷新率。</p>}
    </Collapsible>
  </Surface>;
}
