import { applyAirplane, restoreAirplane } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
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
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="airplane" />飞行模式</h2>
      <div className="ccc-module-tools">
        {(error || profile?.error || profile?.restoreError) && <DetailsDialog title="飞行模式诊断" label="详情">
          {Array.from(new Set([error, profile?.error, profile?.restoreError].filter(Boolean))).map(issue => <p key={issue}>{issue}</p>)}
        </DetailsDialog>}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取飞行模式" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status" aria-label="系统飞行模式"><AirplaneArtwork /><div>
      <strong>{current ? text(current.enabled) : "状态未知"}</strong>
      {(error || profile?.error || profile?.restoreError) && <p className="ccc-control-caption" role="status">查看诊断或重新读取</p>}
    </div></div>
    <SegmentedControl className="ccc-module-control-end" label="飞行模式目标" value={choice} options={options} disabled={busy || !current || !profile?.revision} onChange={setChoice} />
    <ConfigurationAction label="飞行模式" revision={JSON.stringify(request)} together disabled={busy || !request || !current || current.enabled === enabled}
      onConfirm={() => request ? run(() => applyAirplane(request), "飞行模式已应用") : Promise.resolve(false)}>
      {current && <p>{text(current.enabled)} → {text(enabled)}</p>}
      {enabled && <p className="ccc-control-caption">开启后可能断开无线连接。</p>}
    </ConfigurationAction>
    <ConfigurationAction label="飞行模式" revision={profile?.restoreRevision ?? ""} restore disabled={busy || !profile?.restoreRevision}
      onConfirm={() => profile?.restoreRevision ? run(() => restoreAirplane(profile.restoreRevision!), "飞行模式初始状态已恢复") : undefined}>
      {profile?.initial && <p>恢复至{ text(profile.initial.enabled) }。</p>}
    </ConfigurationAction>
  </Surface>;
}
