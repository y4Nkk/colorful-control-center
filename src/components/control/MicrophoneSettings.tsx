import { Icon } from "../Icon";
import { applyMicrophones, restoreMicrophones } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { useMicrophoneSettings } from "./useMicrophoneSettings";

const stateText = (muted: boolean) => muted ? "已静音" : "未静音";
const options = [{ value: "keep", label: "保留当前" }, { value: "mute", label: "全部静音" }, { value: "unmute", label: "全部取消静音" }];
export function MicrophoneSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, choice, setChoice, busy, error, run, reread } = useMicrophoneSettings(active, revision);
  const muted = choice === "mute", endpoints = profile?.current.endpoints ?? [];
  const dirty = choice !== "keep" && !!profile && (profile.current.saved !== Number(muted) || endpoints.some(endpoint => endpoint.muted !== muted));
  const request = profile && choice !== "keep" ? { expectedRevision: profile.revision, muted } : null;
  const facts = request ? endpoints.flatMap((endpoint, index) => endpoint.muted === muted ? [] : [{ label: endpoint.name || `麦克风 ${index + 1}`, value: `${stateText(endpoint.muted)} → ${stateText(muted)}` }]) : [];
  if (request && profile && profile.current.saved !== Number(muted)) facts.push({ label: "保存静音偏好", value: `${profile.current.saved > 1 ? "未识别" : stateText(profile.current.saved !== 0)} → ${stateText(muted)}` });
  return <Surface as="section" variant="group" className="ccc-panel ccc-microphones" aria-label="麦克风静音">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="microphone" />麦克风</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取麦克风</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取麦克风状态，请重试。" : "正在读取麦克风状态。"}</p> : <>
      <div className="ccc-microphone-grid">
        <Surface as="section" variant="inset" className="ccc-profile-values" aria-label="当前麦克风状态"><strong>当前状态</strong>
          {!endpoints.length && <p className="ccc-control-caption">没有活动麦克风</p>}
          {endpoints.map((endpoint, index) => <div className="ccc-microphone-endpoint" key={endpoint.id}><span>{endpoint.name || `麦克风 ${index + 1}`}</span><span className="ccc-control-caption">{stateText(endpoint.muted)}</span></div>)}
        </Surface>
        <Surface as="section" variant="inset" className="ccc-profile-values" aria-label="麦克风目标"><strong>目标设置</strong>
          <SegmentedControl label="麦克风目标" value={choice} options={options} disabled={busy || !!profile.blockedReason || !endpoints.length} onChange={setChoice} />
          {profile.blockedReason && <p className="ccc-control-error">{profile.blockedReason}</p>}
        </Surface>
      </div>
      {facts.length > 0 && <FactList compact facts={facts} />}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify(request)} label="应用麦克风设置" confirmLabel="确认修改这些麦克风？" disabled={!request || !dirty || !!profile.blockedReason || !endpoints.length} busy={busy}
          onConfirm={() => request ? run(() => applyMicrophones(request), "麦克风设置已应用") : undefined} />
        <ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复麦克风初始" confirmLabel="恢复各麦克风修改前的状态？" disabled={!profile.canRestore} busy={busy}
          onConfirm={() => run(() => restoreMicrophones(profile.revision), "麦克风初始状态已恢复")} />
      </div>
      {profile.initial && <p className="ccc-control-caption">恢复目标：{profile.initial.endpoints.map((endpoint, index) => `${endpoint.name || `麦克风 ${index + 1}`} ${stateText(endpoint.muted)}`).join("；")}</p>}
      {profile.restoreBlockedReason && <p className="ccc-control-error">恢复暂不可用：{profile.restoreBlockedReason}</p>}
      {profile.blockedReason && profile.canRestore && <p className="ccc-control-caption">可先恢复初始设备；新接入的设备保持当前状态。</p>}
    </>}
  </Surface>;
}
