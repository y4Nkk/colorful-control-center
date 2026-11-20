import { useId } from "react";
import { enableAutomaticRefresh, OEM_POWER_MODES, readAutomaticRefresh, restoreAutomaticRefresh, type AutomaticRefreshFacts } from "../../api/client";
import { useAutomaticDisplayTrialId } from "../../app/automatic-display-trial";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { useDeviceSettings } from "./useDeviceSettings";

export function AutomaticRefreshSettings({ active, revision }: { active: boolean; revision: number }) {
  const trialId = useAutomaticDisplayTrialId();
  const { profile, busy, error, reread, run } = useDeviceSettings(readAutomaticRefresh, active, `${revision}:${trialId}`, 1000);
  const hintId = useId();
  const facts = profile?.current;
  const enabled = !!profile?.active;
  const issues = Array.from(new Set([profile?.blockedReason, error, profile?.error, profile?.issue].filter((issue): issue is string => !!issue)));
  const status = busy ? "正在应用…" : !enabled && profile?.initial ? "已暂停" : !profile && !error ? "读取中…" : null;
  const mayChange = enabled ? profile?.canRestore : profile?.canStart;
  const preview = profile?.preview;
  const hint = enabled && profile?.initialDisplay ? `停用时恢复 ${profile.initialDisplay.frequency} Hz`
    : !enabled && mayChange && preview && preview.before.frequency !== preview.target.frequency
      ? `启用时试用 ${preview.target.frequency} Hz` : null;

  const setEnabled = (next: boolean) => {
    if (!profile || !mayChange || next === enabled) return;
    const action = next ? enableAutomaticRefresh : restoreAutomaticRefresh;
    void run(() => action(profile.revision));
  };

  return <Surface as="section" variant="group" className="ccc-panel" aria-label="自动刷新率" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="display" />自动刷新率</h2>
      <div className="ccc-module-tools"><DetailsDialog title="刷新率规则与恢复" label="详情">
        {facts && <FactList compact facts={[{ label: "候选刷新率", value: `${facts.candidates.join(" / ")} Hz` }]} />}
        <p>{facts?.customer === 5 ? "安静或省电时降低刷新率，离开后恢复。"
          : "电池省电或静音增强时降低刷新率，按供电与模式恢复。"}</p>
        <p>刷新率切换后有 15 秒试用时间。停用时恢复启用前的显示与保存设置。</p>
        {facts?.candidates.some(rate => rate > 255) && <p>原厂保存字段上限为 255 Hz，恢复时可能选择最高候选。</p>}
        {facts?.supplyModeSwitch && <p>供电切换时，等待性能模式更新后调整。</p>}
        {issues.map(issue => <p key={issue} className="ccc-control-error">{issue}</p>)}
      </DetailsDialog>
      {issues.length > 0 && <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取自动刷新率"
        disabled={busy} onClick={reread} />}</div>
    </div>
    <div className="ccc-refresh-control">
      <div><strong>按供电与模式切换</strong>
        {status && <span className="ccc-control-caption" role="status">{status}</span>}
        {hint && <span id={hintId} className="ccc-control-caption">{hint}</span>}
      </div>
      <Switch label="启用自动刷新率" describedBy={hint ? hintId : undefined} checked={enabled}
        disabled={busy || !mayChange} onChange={setEnabled} />
    </div>
    <AutomaticRefreshReadout facts={facts} />
    {issues.length > 0 && <p className="ccc-control-error" role="alert">{issues[0]}</p>}
    <OemConflictAction issues={issues} disabled={busy} onClosed={reread} />
    {profile?.canRestore && !enabled && <div className="ccc-actions">
      <Button size="compact" icon="undo" variant="secondary" disabled={busy}
        onClick={() => void run(() => restoreAutomaticRefresh(profile.revision))}>恢复启用前状态</Button>
    </div>}
  </Surface>;
}

function AutomaticRefreshReadout({ facts }: { facts?: AutomaticRefreshFacts | null }) {
  const supply = facts?.onAc === true ? "已插电" : facts?.onAc === false ? "电池供电" : "供电未知";
  return <div className="ccc-refresh-readout">
    <div className="ccc-refresh-screen">
      <span className="ccc-control-caption">当前主屏</span>
      <strong className="ccc-mono">{facts ? <>{facts.display.frequency} <span>Hz</span></> : "—"}</strong>
      {facts && <span className="ccc-refresh-source">
        <Icon name={facts.onAc === true ? "plug" : facts.onAc === false ? "battery" : "power"} size={16} />
        {supply} · {OEM_POWER_MODES[facts.mode] ?? `模式 ${facts.mode}`}
      </span>}
    </div>
    {facts && <dl className="ccc-refresh-rates">
      <div><dt><Icon name="battery" size={16} />低刷新率</dt><dd className="ccc-mono">{facts.low} Hz</dd></div>
      <div><dt><Icon name="refresh" size={16} />恢复刷新率</dt><dd className="ccc-mono">{facts.savedOrHighest} Hz</dd></div>
    </dl>}
  </div>;
}
