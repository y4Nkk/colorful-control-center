import { applyMicrophones, readMicrophones, restoreMicrophones, type MicrophoneSnapshot } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { InputDeviceVisual } from "./InputDeviceVisual";
import { useDeviceSettings } from "./useDeviceSettings";

const stateText = (muted: boolean) => muted ? "已静音" : "未静音";
const savedText = (saved: number) => saved > 1 ? "未识别" : stateText(saved !== 0);
const options = [{ value: "true", label: "全部静音" }, { value: "false", label: "取消静音" }];
const TITLE = "麦克风静音";

function microphoneFacts(snapshot: MicrophoneSnapshot) {
  return [
    ...snapshot.endpoints.map((endpoint, index) => ({
      label: endpoint.name || `麦克风 ${index + 1}`, value: stateText(endpoint.muted),
    })),
    { label: "保存静音偏好", value: savedText(snapshot.saved) },
  ];
}

export function MicrophoneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readMicrophones, active, revision, 5000);
  const current = profile?.current, initial = profile?.initial;
  const endpoints = current?.endpoints ?? [];
  const mutedCount = endpoints.filter(endpoint => endpoint.muted).length;
  const canChange = endpoints.length > 0 && !profile?.blockedReason;
  const value = current && endpoints.length > 0 && current.saved <= 1
    && mutedCount === (current.saved === 1 ? endpoints.length : 0) ? String(current.saved === 1) : "";
  const summary = value ? null : !current ? "状态未知" : !endpoints.length ? "无活动麦克风"
    : mutedCount === 0 ? "未静音" : mutedCount === endpoints.length ? "全部静音" : "部分静音";
  const change = (next: string) => {
    if (!profile || !canChange || next === value) return;
    const request = { expectedRevision: profile.revision, muted: next === "true" };
    void run(() => applyMicrophones(request), "麦克风设置已应用");
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="microphone-heading" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 id="microphone-heading" className="ccc-panel-title"><Icon name="microphone" />{TITLE}</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="麦克风详情" label="详情">
          {current && <FactList compact facts={microphoneFacts(current)} />}
          {profile?.blockedReason && <p>{profile.blockedReason}</p>}
          {profile?.restoreBlockedReason && profile.restoreBlockedReason !== profile.blockedReason && <p>恢复暂不可用 · {profile.restoreBlockedReason}</p>}
          {error && <p className="ccc-control-error">{error}</p>}
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取麦克风" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status" aria-label="当前麦克风状态">
      <InputDeviceVisual kind="microphone" />
      <div>{summary && <strong>{summary}</strong>}{endpoints.length > 0 && <p className="ccc-control-caption">{endpoints.length} 个设备</p>}
        {profile?.blockedReason && <p className="ccc-control-caption">{profile.canRestore ? "可先恢复初始设备" : "更改暂不可用，查看详情"}</p>}</div>
    </div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    <SegmentedControl className="ccc-module-control-end" label={TITLE} value={value} options={options} disabled={busy || !canChange} onChange={change} />
    {profile?.canRestore && <div className="ccc-actions">
      <ConfirmButton key={profile.revision} label="恢复初始设置" confirmLabel="确认恢复" size="compact" icon="undo" busy={busy}
        onConfirm={() => run(() => restoreMicrophones(profile.revision), "麦克风初始状态已恢复")}>
        {initial && <FactList compact facts={microphoneFacts(initial)} />}
        {profile.blockedReason && <p className="ccc-control-caption">只恢复初始设备，新接入的设备保持当前状态。</p>}
      </ConfirmButton>
    </div>}
  </Surface>;
}
