import { useId } from "react";
import { enableAutomaticRefresh, readAutomaticRefresh, restoreAutomaticRefresh, type AutomaticRefreshFacts } from "../../api/client";
import { useAutomaticDisplayTrialId } from "../../app/automatic-display-trial";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
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
  const waiting = busy ? "正在应用…" : !profile && !error ? "正在读取…" : null;
  const status = held && !enabled ? "已暂停" : null;
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
        {facts && <FactList compact facts={[{ label: "可选刷新率", value: `${facts.candidates.join(" / ")} Hz` }]} />}
        <p>{facts?.customer === 5 ? "切到「安静」或「省电」时降低刷新率，切回「娱乐」或「性能」时恢复。"
          : "用电池且预设模式为「省电」时降低刷新率。插拔电源或切换模式时，软件会按电脑的规则选择合适的刷新率。"}</p>
        <p>每次自动切换刷新率后有 15 秒试用时间。关闭自动刷新率时，会把屏幕设置和电脑保存的设置恢复成启用前的样子。</p>
        {facts?.candidates.some(rate => rate > 255) && <p>电脑能保存的刷新率最高为 255 Hz。恢复时，若保存值不在可选列表里，就会用屏幕支持的最高可选刷新率。</p>}
        {facts?.supplyModeSwitch && <p>插拔电源时不直接调整，等预设模式切换完成后再调整。</p>}
        {issues.map(issue => <p key={issue} className="ccc-control-error"><Icon name="error" size={16} />{issue}</p>)}
      </DetailsDialog>
      {issues.length > 0 && <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取自动刷新率"
        disabled={busy} onClick={reread} />}</div>
    </div>
    <AutomaticRefreshReadout facts={facts} active={active} />
    {issues.length > 0 && <p className="ccc-control-error" role="alert">{issues[0]}</p>}
    <OemConflictAction issues={issues} disabled={busy} onClosed={reread} />
    <div className="ccc-refresh-control">
      <div><strong>按供电与模式切换</strong>
        {waiting ? <Loading label={waiting} /> : status && <span className="ccc-control-caption" role="status">{status}</span>}
        {hint && <span id={hintId} className="ccc-control-caption">{hint}</span>}
      </div>
      <Switch label="启用自动刷新率" describedBy={hint ? hintId : undefined} checked={held}
        disabled={busy || !mayChange} onChange={setEnabled} />
    </div>
  </Surface>;
}

/** 扫描纹理是纯装饰：节奏固定，不表示实际刷新时序。 */
function RefreshScanArtwork({ active }: { active: boolean }) {
  const id = `ccc-scan-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const upperFold = "M0 104C48 107 68 83 107 56c30-21 66-20 93-48";
  const lowerFold = "M29 112c42-7 61-32 95-43 26-8 53-8 76-27";
  return <svg className="ccc-refresh-scan" aria-hidden="true" focusable="false" data-active={active || undefined}>
    <defs>
      <radialGradient id={`${id}-glass`} cx=".84" cy=".31" r=".79">
        <stop stopColor="var(--ccc-display-art-ice)" stopOpacity=".52" />
        <stop offset=".54" stopColor="var(--ccc-display-art-blue)" stopOpacity=".21" />
        <stop offset="1" stopColor="var(--ccc-display-art-violet)" stopOpacity="0" />
      </radialGradient>
      <linearGradient id={`${id}-facet`} x1=".1" y1="1" x2=".88" y2="0">
        <stop stopColor="var(--ccc-display-art-blue)" stopOpacity=".03" />
        <stop offset=".57" stopColor="var(--ccc-display-art-violet)" stopOpacity=".28" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" stopOpacity=".58" />
      </linearGradient>
      <pattern id={`${id}-lines`} width="7" height="7" patternUnits="userSpaceOnUse">
        <rect y="6" width="7" height="1" fill="var(--ccc-display-art-blue)" fillOpacity=".09" />
        <rect width="7" height=".5" fill="var(--ccc-display-art-ice)" fillOpacity=".17" />
      </pattern>
      <linearGradient id={`${id}-trail`} x1="0" y1="0" x2="0" y2="1">
        <stop stopColor="var(--ccc-display-art-blue)" stopOpacity="0" />
        <stop offset=".75" stopColor="var(--ccc-display-art-blue)" stopOpacity=".18" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" stopOpacity=".42" />
      </linearGradient>
      <linearGradient id={`${id}-edge`}>
        <stop stopColor="var(--ccc-display-art-blue)" stopOpacity="0" />
        <stop offset=".68" stopColor="var(--ccc-display-art-blue)" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" />
      </linearGradient>
    </defs>
    <rect width="100%" height="100%" fill={`url(#${id}-glass)`} />
    <svg viewBox="0 0 200 112" width="100%" height="100%" preserveAspectRatio="none">
      <path d={`${upperFold}V112H0Z`} fill={`url(#${id}-facet)`} />
      <path d={`${lowerFold}V112Z`} fill={`url(#${id}-facet)`} opacity=".62" />
      <g fill="none" stroke={`url(#${id}-edge)`} strokeWidth="1.1">
        <path d={upperFold} strokeOpacity=".68" vectorEffect="non-scaling-stroke" />
        <path d={lowerFold} strokeOpacity=".4" vectorEffect="non-scaling-stroke" />
      </g>
    </svg>
    <rect width="100%" height="100%" fill={`url(#${id}-lines)`} />
    <g className="ccc-refresh-scan-beam">
      <rect className="ccc-refresh-scan-trail" y="-26" width="100%" height="26" fill={`url(#${id}-trail)`} />
      <rect className="ccc-refresh-scan-edge" y="-1.5" width="100%" height="1.5" fill={`url(#${id}-edge)`} />
    </g>
    <g className="ccc-refresh-scan-pulses" fill="none" stroke="var(--ccc-display-art-blue)" strokeWidth="1.5" strokeLinecap="round">
      {[23.5, 47.5, 71.5].map(y => <line key={y} className="ccc-refresh-scan-pulse" x1="0" x2="100%" y1={y} y2={y} pathLength="100" />)}
    </g>
  </svg>;
}

function RefreshTargetsArtwork() {
  const id = `ccc-refresh-targets-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const upperFold = "M0 24C45 26 75 24 111 16s57-6 88 1 59 6 89-2S361 7 400 2";
  const lowerFold = "M0 28C45 30 80 29 116 22s53-6 84 1 58 7 89 0 71-7 111-13";
  return <svg className="ccc-refresh-target-art" viewBox="0 0 400 32" preserveAspectRatio="none"
    aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-glass`} x1=".08" y1="1" x2=".95" y2="0">
        <stop stopColor="var(--ccc-display-art-blue)" stopOpacity=".05" />
        <stop offset=".54" stopColor="var(--ccc-display-art-blue)" stopOpacity=".19" />
        <stop offset=".78" stopColor="var(--ccc-display-art-violet)" stopOpacity=".25" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" stopOpacity=".42" />
      </linearGradient>
      <linearGradient id={`${id}-rim`}>
        <stop stopColor="var(--ccc-display-art-blue)" stopOpacity=".15" />
        <stop offset=".38" stopColor="var(--ccc-display-art-blue)" stopOpacity=".46" />
        <stop offset=".7" stopColor="var(--ccc-display-art-ice)" stopOpacity=".68" />
        <stop offset="1" stopColor="var(--ccc-display-art-violet)" stopOpacity=".52" />
      </linearGradient>
    </defs>
    <path d={`${upperFold}V32H0Z`} fill={`url(#${id}-glass)`} />
    <path d={`${lowerFold}V32H0Z`} fill={`url(#${id}-glass)`} opacity=".74" />
    <g fill="none" stroke={`url(#${id}-rim)`} strokeWidth="1.1">
      <path d={upperFold} vectorEffect="non-scaling-stroke" />
      <path d={lowerFold} opacity=".56" vectorEffect="non-scaling-stroke" />
    </g>
  </svg>;
}

function AutomaticRefreshReadout({ facts, active }: { facts?: AutomaticRefreshFacts | null; active: boolean }) {
  return <div className="ccc-refresh-readout">
    <div className="ccc-refresh-screen">
      <RefreshScanArtwork active={active && !!facts} />
      <span className="ccc-control-caption">当前主屏</span>
      <strong className="ccc-mono">{facts ? <>{facts.display.frequency} <span>Hz</span></> : "—"}</strong>
    </div>
    {facts && <div className="ccc-refresh-targets">
      <RefreshTargetsArtwork />
      <dl className="ccc-refresh-rates">
        <div><dt>低刷新率</dt><dd className="ccc-mono">{facts.low} <span>Hz</span></dd></div>
        <div><dt>恢复刷新率</dt><dd className="ccc-mono">{facts.savedOrHighest} <span>Hz</span></dd></div>
      </dl>
    </div>}
  </div>;
}
