import { useId } from "react";
import { applyAirplane, readAirplane } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { AirplaneArtwork } from "./AirplaneArtwork";
import { useDeviceSettings } from "./useDeviceSettings";

export function AirplaneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, loaded, busy, error, run, reread } = useDeviceSettings(readAirplane, active, revision, 5000);
  const current = profile?.current, expectedRevision = profile?.revision;
  const blocked = profile?.blockedReason;
  const hintId = useId();
  const issues = Array.from(new Set([blocked, error, profile?.error].filter((issue): issue is string => !!issue)));
  const change = (enabled: boolean) => {
    if (!expectedRevision || !current || blocked || current.enabled === enabled) return;
    void run(() => applyAirplane({ expectedRevision, enabled }), "飞行模式已应用");
  };

  return <Surface as="section" variant="group" className="ccc-panel ccc-airplane" aria-label="飞行模式" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="airplane" />飞行模式</h2>
      <div className="ccc-module-tools">
        {issues.length > 0 && <DetailsDialog title="飞行模式诊断" label="详情">
          {!current && <p>要确认 Windows 当前的飞行模式，请关闭此弹窗，点「重新读取飞行模式」。</p>}
          {issues.map(issue => <p key={issue}>{issue}</p>)}
        </DetailsDialog>}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取飞行模式" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status ccc-airplane-switch" aria-label="系统飞行模式">
      <AirplaneArtwork />
      {!loaded ? <Loading label="正在读取飞行模式…" /> : !current && <strong>状态未知</strong>}
      {current?.enabled === false && <p id={hintId} className="ccc-control-caption">开启后可能断开无线连接</p>}
      <Switch label="飞行模式" describedBy={current?.enabled === false ? hintId : undefined}
        checked={!!current?.enabled} disabled={busy || !current || !expectedRevision || !!blocked} onChange={change} />
    </div>
    {issues.length > 0 && <p className="ccc-control-error" role="alert">{issues[0]}</p>}
    <OemConflictAction issues={issues} disabled={busy} onClosed={reread} />
  </Surface>;
}
