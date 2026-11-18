import { Icon } from "../Icon";
import { applyTouchpad, restoreTouchpad } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { InputDeviceVisual } from "./InputDeviceVisual";
import { useTouchpadSettings } from "./useTouchpadSettings";

const stateText = (enabled: boolean | null) => enabled === null ? "未知" : enabled ? "已启用" : "已关闭";
const savedText = (saved: number) => saved > 1 ? "未识别" : stateText(saved !== 0);
const options = [{ value: "keep", label: "保留当前" }, { value: "on", label: "启用" }, { value: "off", label: "关闭" }];

export function TouchpadSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, choice, setChoice, busy, error, run, reread } = useTouchpadSettings(active, revision);
  const enabled = choice === "on", current = profile?.current, initial = profile?.initial;
  const request = profile && choice !== "keep" ? { expectedRevision: profile.revision, enabled } : null;
  const facts = request && current ? [
    ...(current.enabled !== enabled ? [{ label: "系统触摸板", value: `${stateText(current.enabled)} → ${stateText(enabled)}` }] : []),
    ...(current.saved !== Number(enabled) ? [{ label: "保存开关偏好", value: `${savedText(current.saved)} → ${stateText(enabled)}` }] : []),
  ] : [];
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="触摸板设置">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="touchpad" />触摸板</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="触摸板详情" label="详情">
          {current && <FactList compact facts={[{ label: "系统触摸板", value: stateText(current.enabled) }, { label: "保存偏好", value: savedText(current.saved) }]} />}
          {profile?.blockedReason && <p>{profile.blockedReason}</p>}
          {profile?.restoreBlockedReason && profile.restoreBlockedReason !== profile.blockedReason && <p>恢复暂不可用 · {profile.restoreBlockedReason}</p>}
          {error && <p className="ccc-control-error">{error}</p>}
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取触摸板" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status" aria-label="当前触摸板状态">
      <InputDeviceVisual kind="touchpad" />
      <div><strong>{stateText(current?.enabled ?? null)}</strong>
        {profile?.blockedReason && <p className="ccc-control-caption">更改暂不可用，查看详情</p>}</div>
    </div>
    {error && <p role="alert" className="ccc-control-error">触摸板操作失败，详情中可查看原因。</p>}
    {profile && <>
      <SegmentedControl className="ccc-module-control-end" label="触摸板目标" value={choice} options={options} disabled={busy || !!profile.blockedReason} onChange={setChoice} />
      <ConfigurationAction label="触摸板" revision={JSON.stringify(request)} together
        disabled={busy || !request || !facts.length || !!profile.blockedReason}
        onConfirm={() => request ? run(() => applyTouchpad(request), "触摸板设置已应用") : Promise.resolve(false)}>
        <FactList compact facts={facts} />
        {!enabled && <p className="ccc-control-caption">关闭后可使用键盘或外接鼠标。</p>}
      </ConfigurationAction>
      <ConfigurationAction label="触摸板" revision={profile.revision} restore disabled={busy || !profile.canRestore}
        onConfirm={() => run(() => restoreTouchpad(profile.revision), "触摸板初始状态已恢复")}>
        {initial && <FactList compact facts={[
          { label: "系统触摸板", value: stateText(initial.enabled) }, { label: "保存偏好", value: savedText(initial.saved) },
        ]} />}
        {profile.blockedReason && <p className="ccc-control-caption">只恢复初始设备，新接入的设备保持当前状态。</p>}
      </ConfigurationAction>
    </>}
  </Surface>;
}
