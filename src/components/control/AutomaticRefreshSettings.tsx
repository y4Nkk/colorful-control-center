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
  // 启用记录仍在：策略运行中，或已暂停、事务未完成而等待用户关闭。
  const held = profile?.initial != null;
  const issues = Array.from(new Set([profile?.blockedReason, error, profile?.error, profile?.issue].filter((issue): issue is string => !!issue)));
  const status = busy ? "正在应用…" : held && !enabled ? "已暂停" : !profile && !error ? "读取中…" : null;
  const mayChange = held ? profile?.canRestore : profile?.canStart;
  const preview = profile?.preview;
  const hint = held && profile?.initialDisplay ? `停用时恢复 ${profile.initialDisplay.frequency} Hz`
    : !held && mayChange && preview && preview.before.frequency !== preview.target.frequency
      ? `启用时试用 ${preview.target.frequency} Hz` : null;

  const setEnabled = (next: boolean) => {
    if (!profile || !mayChange || next === held) return;
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
        {issues.map(issue => <p key={issue} className="ccc-control-error"><Icon name="error" size={16} />{issue}</p>)}
      </DetailsDialog>
      {issues.length > 0 && <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取自动刷新率"
        disabled={busy} onClick={reread} />}</div>
    </div>
    <div className="ccc-refresh-control">
      <div><strong>按供电与模式切换</strong>
        {status && <span className="ccc-control-caption" role="status">{status}</span>}
        {hint && <span id={hintId} className="ccc-control-caption">{hint}</span>}
      </div>
      <Switch label="启用自动刷新率" describedBy={hint ? hintId : undefined} checked={held}
        disabled={busy || !mayChange} onChange={setEnabled} />
    </div>
    <AutomaticRefreshReadout facts={facts} active={active} />
    {issues.length > 0 && <p className="ccc-control-error" role="alert">{issues[0]}</p>}
    <OemConflictAction issues={issues} disabled={busy} onClosed={reread} />
  </Surface>;
}

/** 扫描纹理是纯装饰：节奏固定，不表示实际刷新时序。 */
function RefreshScanArtwork({ active }: { active: boolean }) {
  const id = `ccc-scan-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return <svg className="ccc-refresh-scan" aria-hidden="true" focusable="false" data-active={active || undefined}>
    <defs>
      <pattern id={`${id}-lines`} width="6" height="6" patternUnits="userSpaceOnUse">
        <rect y="5" width="6" height="1" fill="var(--ccc-action)" fillOpacity=".11" />
      </pattern>
      <linearGradient id={`${id}-trail`} x1="0" y1="0" x2="0" y2="1">
        <stop stopColor="var(--ccc-action)" stopOpacity="0" />
        <stop offset="1" stopColor="var(--ccc-action)" stopOpacity=".22" />
      </linearGradient>
    </defs>
    <rect width="100%" height="100%" fill={`url(#${id}-lines)`} />
    <g className="ccc-refresh-scan-beam">
      <rect className="ccc-refresh-scan-trail" y="-26" width="100%" height="26" fill={`url(#${id}-trail)`} />
      <rect className="ccc-refresh-scan-edge" y="-1.5" width="100%" height="1.5" fill="var(--ccc-action)" />
    </g>
    <g className="ccc-refresh-scan-pulses" fill="none" stroke="var(--ccc-action)" strokeWidth="1.5" strokeLinecap="round">
      {[23.5, 47.5, 71.5].map(y => <line key={y} className="ccc-refresh-scan-pulse" x1="0" x2="100%" y1={y} y2={y} pathLength="100" />)}
    </g>
  </svg>;
}

function AutomaticRefreshReadout({ facts, active }: { facts?: AutomaticRefreshFacts | null; active: boolean }) {
  const supply = facts?.onAc === true ? "已插电" : facts?.onAc === false ? "电池供电" : "供电未知";
  return <div className="ccc-refresh-readout">
    <div className="ccc-refresh-screen">
      <RefreshScanArtwork active={active && !!facts} />
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
