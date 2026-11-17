import { Icon } from "../Icon";
import { applyTouchpad, restoreTouchpad } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { useTouchpadSettings } from "./useTouchpadSettings";

const stateText = (enabled: boolean | null) => enabled === null ? "未知" : enabled ? "已启用" : "已关闭";
const savedText = (saved: number) => saved > 1 ? "未识别" : stateText(saved !== 0);
const options = [{ value: "keep", label: "保留当前" }, { value: "on", label: "启用" }, { value: "off", label: "关闭" }];
export function TouchpadSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, choice, setChoice, busy, error, run, reread } = useTouchpadSettings(active, revision);
  const enabled = choice === "on", current = profile?.current;
  const request = profile && choice !== "keep" ? { expectedRevision: profile.revision, enabled } : null;
  const facts = request && current ? [
    ...(current.enabled !== enabled ? [{ label: "系统触摸板", value: `${stateText(current.enabled)} → ${stateText(enabled)}` }] : []),
    ...(current.saved !== Number(enabled) ? [{ label: "保存开关偏好", value: `${savedText(current.saved)} → ${stateText(enabled)}` }] : []),
  ] : [];
  return <Surface as="section" variant="group" className="ccc-panel ccc-touchpad" aria-label="触摸板设置">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="touchpad" />触摸板</h2><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取触摸板</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取触摸板状态，请重试。" : "正在读取触摸板状态。"}</p> : <>
      <div className="ccc-touchpad-grid">
        <Surface as="section" variant="inset" className="ccc-profile-values" aria-label="当前触摸板状态"><strong>当前状态</strong>
          <FactList compact facts={[{ label: "系统触摸板", value: stateText(profile.current.enabled) }, { label: "保存偏好", value: savedText(profile.current.saved) }]} />
        </Surface>
        <Surface as="section" variant="inset" className="ccc-profile-values" aria-label="触摸板目标"><strong>目标设置</strong>
          <SegmentedControl label="触摸板目标" value={choice} options={options} disabled={busy || !!profile.blockedReason} onChange={setChoice} />
          {choice === "off" && <span className="ccc-chip is-warn">关闭后使用键盘或外接鼠标</span>}
          {profile.blockedReason && <p className="ccc-control-error">{profile.blockedReason}</p>}
        </Surface>
      </div>
      {facts.length > 0 && <FactList compact facts={facts} />}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify(request)} label="应用触摸板设置" confirmLabel={`确认${enabled ? "启用" : "关闭"}触摸板？`} disabled={!request || !facts.length || !!profile.blockedReason} busy={busy}
          onConfirm={() => request ? run(() => applyTouchpad(request), "触摸板设置已应用") : undefined} />
        <ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复触摸板初始" confirmLabel="恢复修改前的触摸板状态与偏好？" disabled={!profile.canRestore} busy={busy}
          onConfirm={() => run(() => restoreTouchpad(profile.revision), "触摸板初始状态已恢复")} />
      </div>
      {profile.initial && <p className="ccc-control-caption">恢复目标：系统{stateText(profile.initial.enabled)}；保存偏好{savedText(profile.initial.saved)}</p>}
      {profile.restoreBlockedReason && <p className="ccc-control-error">恢复暂不可用：{profile.restoreBlockedReason}</p>}
    </>}
  </Surface>;
}
