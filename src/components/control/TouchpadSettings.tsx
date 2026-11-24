import { applyTouchpad, readTouchpad } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { OemConflictAction } from "../OemConflictAction";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { InputDeviceVisual } from "./InputDeviceVisual";
import { useDeviceSettings } from "./useDeviceSettings";

const stateText = (enabled: boolean | null) => enabled === null ? "未知" : enabled ? "已启用" : "已关闭";
const savedText = (saved: number) => saved > 1 ? "未识别" : stateText(saved !== 0);
const options = [{ value: "true", label: "启用" }, { value: "false", label: "关闭", title: "关闭后可使用键盘或外接鼠标" }];

export function TouchpadSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readTouchpad, active, revision, 5000);
  const current = profile?.current;
  const canChange = current?.enabled != null && !profile?.blockedReason;
  const value = current && current.enabled !== null && current.saved === Number(current.enabled) ? String(current.enabled) : "";
  const summary = value ? null : current?.enabled == null ? "系统状态未知" : `系统${stateText(current.enabled)}`;
  const change = (next: string) => {
    if (!profile || !canChange || next === value) return;
    const request = { expectedRevision: profile.revision, enabled: next === "true" };
    void run(() => applyTouchpad(request), "触摸板设置已应用");
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="touchpad-heading" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 id="touchpad-heading" className="ccc-panel-title"><Icon name="touchpad" />触摸板</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="触摸板详情" label="详情">
          {current && <FactList compact facts={[{ label: "系统触摸板", value: stateText(current.enabled) }, { label: "保存偏好", value: savedText(current.saved) }]} />}
          {profile?.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
          {error && <p className="ccc-control-error"><Icon name="error" size={16} />{error}</p>}
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取触摸板" disabled={busy} onClick={reread} />
      </div>
    </div>
    <div className="ccc-device-status" aria-label="当前触摸板状态">
      <InputDeviceVisual kind="touchpad" />
      {summary && <strong>{summary}</strong>}
    </div>
    {profile?.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    <OemConflictAction issues={[error, profile?.blockedReason]} disabled={busy} onClosed={reread} />
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    <SegmentedControl className="ccc-module-control-end" label="触摸板" value={value} options={options} disabled={busy || !canChange} onChange={change} />
  </Surface>;
}
