import { useId } from "react";
import { applyAirplane, readAirplane, restoreAirplane } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { AirplaneArtwork } from "./AirplaneArtwork";
import { useDeviceSettings } from "./useDeviceSettings";

const stateText = (enabled: boolean) => enabled ? "已开启" : "已关闭";

export function AirplaneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readAirplane, active, revision, 5000);
  const current = profile?.current, expectedRevision = profile?.revision, restoreRevision = profile?.restoreRevision;
  const hintId = useId();
  const issues = Array.from(new Set([error, profile?.error, profile?.restoreError].filter((issue): issue is string => !!issue)));
  const change = (enabled: boolean) => {
    if (!expectedRevision || !current || current.enabled === enabled) return;
    void run(() => applyAirplane({ expectedRevision, enabled }), "飞行模式已应用");
  };

  return <Surface as="section" variant="group" className="ccc-panel ccc-airplane" aria-label="飞行模式" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="airplane" />飞行模式</h2>
      <div className="ccc-module-tools">
        {issues.length > 0 && <DetailsDialog title="飞行模式诊断" label="详情">
          {issues.map(issue => <p key={issue}>{issue}</p>)}
        </DetailsDialog>}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取飞行模式" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status ccc-airplane-switch" aria-label="系统飞行模式">
      <AirplaneArtwork />
      {!current && <strong>状态未知</strong>}
      {current?.enabled === false && <p id={hintId} className="ccc-control-caption">开启后可能断开无线连接</p>}
      <Switch label="飞行模式" describedBy={current?.enabled === false ? hintId : undefined}
        checked={!!current?.enabled} disabled={busy || !current || !expectedRevision} onChange={change} />
    </div>
    {issues.length > 0 && <p className="ccc-control-error" role="alert">{issues[0]}</p>}
    {restoreRevision && <div className="ccc-actions">
      <ConfirmButton key={restoreRevision} label="恢复初始设置" confirmLabel="确认恢复" size="compact" icon="undo" busy={busy}
        onConfirm={() => run(() => restoreAirplane(restoreRevision), "飞行模式初始状态已恢复")}>
        {profile.initial && <p>恢复至{stateText(profile.initial.enabled)}。</p>}
      </ConfirmButton>
    </div>}
  </Surface>;
}
