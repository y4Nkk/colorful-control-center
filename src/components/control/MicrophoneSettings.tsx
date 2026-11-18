import { Icon } from "../Icon";
import { applyMicrophones, restoreMicrophones, type MicrophoneSnapshot } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { InputDeviceVisual } from "./InputDeviceVisual";
import { useMicrophoneSettings } from "./useMicrophoneSettings";

const stateText = (muted: boolean) => muted ? "已静音" : "未静音";
const savedText = (saved: number) => saved > 1 ? "未识别" : stateText(saved !== 0);
const options = [{ value: "keep", label: "保留当前" }, { value: "mute", label: "全部静音" }, { value: "unmute", label: "取消静音" }];

function microphoneFacts(snapshot: MicrophoneSnapshot) {
  return [
    ...snapshot.endpoints.map((endpoint, index) => ({
      label: endpoint.name || `麦克风 ${index + 1}`, value: stateText(endpoint.muted),
    })),
    { label: "保存静音偏好", value: savedText(snapshot.saved) },
  ];
}

export function MicrophoneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, choice, setChoice, busy, error, run, reread } = useMicrophoneSettings(active, revision);
  const muted = choice === "mute", current = profile?.current, initial = profile?.initial;
  const endpoints = current?.endpoints ?? [];
  const mutedCount = endpoints.filter(endpoint => endpoint.muted).length;
  const summary = !current ? "状态未知" : !endpoints.length ? "无活动麦克风"
    : mutedCount === 0 ? "未静音" : mutedCount === endpoints.length ? "全部静音" : "部分静音";
  const request = profile && choice !== "keep" ? { expectedRevision: profile.revision, muted } : null;
  const facts = request ? endpoints.flatMap((endpoint, index) => endpoint.muted === muted ? [] : [{
    label: endpoint.name || `麦克风 ${index + 1}`, value: `${stateText(endpoint.muted)} → ${stateText(muted)}`,
  }]) : [];
  if (request && current && current.saved !== Number(muted)) facts.push({
    label: "保存静音偏好", value: `${savedText(current.saved)} → ${stateText(muted)}`,
  });
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="麦克风静音">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="microphone" />麦克风</h2>
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
      <div><strong>{summary}</strong>{endpoints.length > 0 && <p className="ccc-control-caption">{endpoints.length} 个设备</p>}
        {profile?.blockedReason && <p className="ccc-control-caption">更改暂不可用，查看详情</p>}</div>
    </div>
    {error && <p role="alert" className="ccc-control-error">麦克风操作失败，详情中可查看原因。</p>}
    {profile && <>
      <SegmentedControl className="ccc-module-control-end" label="麦克风目标" value={choice} options={options} disabled={busy || !!profile.blockedReason || !endpoints.length} onChange={setChoice} />
      <ConfigurationAction label="麦克风" revision={JSON.stringify(request)} together
        disabled={busy || !request || !facts.length || !!profile.blockedReason || !endpoints.length}
        onConfirm={() => request ? run(() => applyMicrophones(request), "麦克风设置已应用") : Promise.resolve(false)}>
        <FactList compact facts={facts} />
      </ConfigurationAction>
      <ConfigurationAction label="麦克风" revision={profile.revision} restore disabled={busy || !profile.canRestore}
        onConfirm={() => run(() => restoreMicrophones(profile.revision), "麦克风初始状态已恢复")}>
        {initial && <FactList compact facts={microphoneFacts(initial)} />}
        {profile.blockedReason && <p className="ccc-control-caption">只恢复初始设备，新接入的设备保持当前状态。</p>}
      </ConfigurationAction>
    </>}
  </Surface>;
}
