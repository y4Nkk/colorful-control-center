import { Icon } from "../Icon";
import { applyHotkeys, restoreHotkeys, resetInvalidHotkeys, type HotkeyScope } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { useHotkeySettings } from "./useHotkeySettings";
import { Collapsible } from "../Collapsible";
import { HotkeyCurrent } from "./HotkeyCurrent";
import { ACTIONS } from "./hotkey-actions";
import { HotkeyEventPreviews } from "./HotkeyEventPreviews";

export function HotkeySettings({ active, revision }: { active: boolean; revision: number }) {
  const model = useHotkeySettings(active, revision), { profile, draft, busy, error, run } = model;
  const blocked = !!draft?.enabled && (!!profile?.facts.blockedReason || !draft.actions.length || draft.actions.some(scope =>
    !profile?.facts.capabilities.find(cap => cap.scope === scope)?.identity));
  const selected = new Set([...(draft?.actions ?? []), ...(profile?.current?.actions ?? [])]);
  const changed = !!profile && !!draft && JSON.stringify(draft) !== JSON.stringify(profile.current);
  const canApply = !!profile?.revision && !!draft && !blocked && (changed || draft.enabled && (!profile.status.running || !profile.authorized));
  const toggle = (scope: HotkeyScope, checked: boolean) => model.setDraft(previous => previous ? {
    ...previous, actions: ACTIONS.filter(action => action.scope === scope ? checked : previous.actions.includes(action.scope)).map(action => action.scope),
  } : previous);
  return <Surface as="section" variant="group" className="ccc-panel ccc-hotkey-panel" aria-label="原厂热键与屏显">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />原厂热键与屏显</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取热键</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile?.recordError && <div className="ccc-mode-clock-facts">
      <p className="ccc-control-error" role="alert">{profile.recordError}；已保存策略未知，未自动修改原文件。</p>
      <p className="ccc-control-caption">清除后停止监听，保存关闭且未选择范围的当前策略；硬件保持本次状态。</p>
      <ConfirmButton icon="delete" key={profile.resetRevision} label="清除不可用热键策略" confirmLabel="清除该文件并停止热键监听？" busy={busy} disabled={!profile.resetRevision}
        onConfirm={() => run(() => resetInvalidHotkeys(profile.resetRevision!), "不可用策略已清除，热键监听已停止", true)} />
    </div>}
    <div className="ccc-hotkey-columns">
      <HotkeyCurrent profile={profile} busy={busy} run={run} screenSelected={selected.has("screenBrightness")} panelSelected={selected.has("panelHotkey")} />
      <Surface variant="inset" className="ccc-hotkey-section">
        {!profile || !draft ? <p className="ccc-control-caption">{error ? "无法读取策略，仍可临时停止监听。" : "正在读取热键能力。"}</p> : <>
          <Checkbox label="启用原厂热键接管" checked={draft.enabled} disabled={busy}
            onChange={event => model.setDraft({ ...draft, enabled: event.target.checked })} />
          <div className="ccc-hotkey-scopes">{ACTIONS.map(action => {
            const cap = profile.facts.capabilities.find(cap => cap.scope === action.scope), checked = draft.actions.includes(action.scope);
            return <div key={action.scope} className="ccc-hotkey-scope">
              <div className="ccc-rule-heading"><Icon name={action.icon} size={16} /><Checkbox label={action.title} checked={checked} disabled={busy || !checked && !cap?.identity}
                onChange={event => toggle(action.scope, event.target.checked)} /></div>
              {!cap?.identity && <p className="ccc-control-caption">{cap?.reason ?? "能力未提供"}</p>}
            </div>;
          })}</div>
          <Checkbox label="显示桌面提示" checked={draft.osd} disabled={busy} onChange={event => model.setDraft({ ...draft, osd: event.target.checked })} />
          <div aria-label="热键策略预览" className="ccc-hotkey-preview">
          <p className="ccc-control-caption">待应用：{draft.enabled ? "启用" : "关闭"} · {draft.actions.length} 项 · 屏显{draft.osd ? "开启" : "关闭"}</p>
          {draft.actions.length > 0 && <Collapsible summary="已选动作说明">
            {ACTIONS.filter(action => draft.actions.includes(action.scope)).map(action => <p key={action.scope}><strong>{action.title}</strong> · {action.rule}</p>)}
          </Collapsible>}
          </div>
          {draft.actions.includes("powerMode") && profile.facts.modeCycle?.nodeChoices.some(choice => choice.action?.target.enabled === false) && <p className="ccc-control-caption">所选循环包含独显关断。</p>}
          {draft.actions.includes("batteryLowAction") && <p className="ccc-control-caption">低电量事件可能请求睡眠或休眠。</p>}
          {draft.actions.includes("panelHotkey") && <p className="ccc-control-caption" aria-label="屏幕按键接管预览">
            接管后：{profile.facts.panelHotkey?.current?.kind === "blankSaver" ? "按键启动空白屏保，正常输入退出" : profile.facts.panelHotkey?.current?.kind === "pointerWake" ? "按键开始等待；键盘按下取消，鼠标活动发送一次唤醒请求" : "当前条件不可用，需要重新读取"}。该行为不显示桌面提示。
          </p>}
          <div className="ccc-actions">
            <ConfirmButton key={`${profile.revision}-${JSON.stringify(draft)}`} label="应用热键策略" confirmLabel="确认此范围的热键接管？" busy={busy} disabled={!canApply}
              onConfirm={() => run(() => applyHotkeys(profile.revision!, draft), "热键软件策略已保存", true)} />
            <ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复初始热键策略" confirmLabel="确认恢复初始软件策略？" busy={busy} disabled={!profile.initial || !profile.revision}
              onConfirm={() => run(() => restoreHotkeys(profile.revision!), "初始软件策略已恢复", true)} />
          </div>
        </>}
      </Surface>
    </div>
    <HotkeyEventPreviews profile={profile} selected={selected} draftActions={draft?.actions ?? []} busy={busy} run={run} />
  </Surface>;
}
