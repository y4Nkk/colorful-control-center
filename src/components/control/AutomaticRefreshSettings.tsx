import { useState } from "react";
import { Icon } from "../Icon";
import { OEM_POWER_MODES } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { useAutomaticRefresh } from "./useAutomaticRefresh";

export function AutomaticRefreshSettings({ active, revision }: { active: boolean; revision: number }) {
  const model = useAutomaticRefresh(active, revision), { view, busy } = model;
  const [draft, setDraft] = useState<boolean | null>(null);
  const facts = view?.current;
  const status = view?.active ? "已启用" : view?.initial ? "已暂停" : view ? "未启用" : model.error ? "读取失败" : "读取中…";
  const supply = facts?.onAc === true ? "已插电" : facts?.onAc === false ? "电池供电" : "供电状态未知";
  const preview = view?.preview;
  const rule = facts?.customer === 5 ? "安静或省电时降低，离开后恢复" : "电池省电或静音增强时降低，按供电与模式恢复";
  const issues = Array.from(new Set([model.error, view?.error, view?.issue].filter(Boolean)));
  const mayChange = view?.active ? view.canRestore : view?.canStart;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="自动刷新率">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />自动刷新率</h2>
      <div className="ccc-module-tools"><DetailsDialog title="刷新率规则与恢复" label="详情">
        {facts && <FactList compact facts={[
          { label: "当前主屏", value: `${facts.display.frequency} Hz` },
          { label: "供电与模式", value: `${supply} · ${OEM_POWER_MODES[facts.mode] ?? `模式 ${facts.mode}`}` },
          { label: "候选刷新率", value: `${facts.candidates.join(" / ")} Hz` },
        ]} />}
        <p>{rule}。试用时限 15 秒，停止后恢复启用前状态。</p>
        {facts?.candidates.some(rate => rate > 255) && <p>原厂保存字段上限为 255 Hz，恢复时可能选择最高候选。</p>}
        {facts?.supplyModeSwitch && <p>供电切换时，等待性能模式更新后调整。</p>}
        {issues.map(issue => <p key={issue} className="ccc-control-error">{issue}</p>)}
      </DetailsDialog>
      {issues.length > 0 && <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取自动刷新率"
        disabled={busy} onClick={() => void model.retry()} />}</div>
    </div>
    <div className="ccc-module-summary">
      <strong className="ccc-mono">{facts ? `${facts.low} / ${facts.savedOrHighest} Hz` : "—"}</strong>
      <span className="ccc-control-caption">{status}{issues.length > 0 ? " · 查看详情" : ""}</span>
    </div>
    <div className="ccc-keyboard-setting ccc-module-control-end"><strong>按供电与模式切换</strong>
      <Switch label="启用自动刷新率" checked={draft ?? !!view?.active} disabled={busy || !mayChange}
        onChange={setDraft} /></div>
    {view && <>
      <ConfigurationAction label="自动刷新率" revision={JSON.stringify([view.revision, draft])}
        disabled={busy || !mayChange || draft === null || draft === view.active}
        onConfirm={async () => {
          const success = await (draft ? model.enable() : model.restore());
          if (success) setDraft(null);
          return success;
        }}>
        {draft ? <>
          <p>{rule}。</p>
          <p>{preview && preview.before.frequency !== preview.target.frequency
            ? `启用时试用 ${preview.before.frequency} → ${preview.target.frequency} Hz，请在 15 秒内保留。`
            : "启用时保持当前频率。"}</p>
        </> : <p>停止自动切换{view.initialDisplay ? `并恢复 ${view.initialDisplay.frequency} Hz` : "并恢复启用前状态"}。</p>}
      </ConfigurationAction>
      <ConfigurationAction label="自动刷新率" revision={view.revision} restore disabled={busy || !view.canRestore}
        onConfirm={async () => { const success = await model.restore(); if (success) setDraft(null); return success; }}>
        <p>停止并恢复{view.initialDisplay ? ` ${view.initialDisplay.frequency} Hz` : "启用前的显示设置"}。</p>
      </ConfigurationAction>
    </>}
  </Surface>;
}
