import { applyAirplane, restoreAirplane } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { SegmentedControl } from "../SegmentedControl";
import { useAirplaneSettings } from "./useAirplaneSettings";
import { AirplaneArtwork } from "./AirplaneArtwork";
const text = (enabled: boolean) => enabled ? "已开启" : "已关闭";
const options = [{ value: "keep", label: "保留当前" }, { value: "on", label: "开启" }, { value: "off", label: "关闭" }];
export function AirplaneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, choice, setChoice, busy, error, run, reread } = useAirplaneSettings(active, revision);
  const current = profile?.current, enabled = choice === "on";
  const request = profile?.revision && choice !== "keep" ? { expectedRevision: profile.revision, enabled } : null;
  return <Surface as="section" variant="group" className="ccc-panel ccc-airplane" aria-label="飞行模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="airplane" />飞行模式</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取飞行模式</Button></div>
    {(error || profile?.error) && <p role="alert" className="ccc-control-error">{[error, profile?.error].filter(Boolean).join("；")}</p>}
    <div className="ccc-airplane-columns">
      <Surface as="section" variant="inset" className="ccc-airplane-state" aria-label="系统飞行模式"><AirplaneArtwork /><div><span className="ccc-control-caption">系统报告</span><strong>{current ? text(current.enabled) : "状态未知"}</strong></div></Surface>
      <Surface as="section" variant="inset" className="ccc-airplane-target" aria-label="飞行模式目标"><strong>目标设置</strong><SegmentedControl label="飞行模式目标" value={choice} options={options} disabled={busy || !current} onChange={setChoice} />{current && request && <span>{text(current.enabled)} → {text(enabled)}</span>}</Surface>
    </div>
    <div className="ccc-actions">
      <ConfirmButton key={JSON.stringify(request)} label="应用飞行模式" confirmLabel="确认切换飞行模式？" busy={busy} disabled={!request || !current || current.enabled === enabled} onConfirm={() => request ? run(() => applyAirplane(request), "飞行模式已应用") : undefined} />
      <ConfirmButton key={profile?.restoreRevision ?? "none"} icon="undo" variant="secondary" label="恢复飞行模式初始" confirmLabel="确认恢复初始飞行模式？" busy={busy} disabled={!profile?.restoreRevision} onConfirm={() => profile?.restoreRevision ? run(() => restoreAirplane(profile.restoreRevision!), "飞行模式初始状态已恢复") : undefined} />
    </div>
    {profile?.initial && <p className="ccc-control-caption">恢复目标：{text(profile.initial.enabled)}</p>}
    {profile?.restoreError && <p className="ccc-control-error">{profile.restoreError}</p>}
  </Surface>;
}
