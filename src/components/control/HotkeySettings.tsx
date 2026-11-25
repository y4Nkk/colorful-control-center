import { Icon } from "../Icon";
import { applyHotkeys, findOemConflict, resetInvalidHotkeys, type HotkeyScope } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ConfigurationAction } from "../ConfigurationAction";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { useHotkeySettings } from "./useHotkeySettings";
import { HotkeyCurrent } from "./HotkeyCurrent";
import { ACTIONS } from "./hotkey-actions";
import { HotkeyEventPreviews } from "./HotkeyEventPreviews";
import { HotkeyRecovery } from "./HotkeyRecovery";

type HotkeyModel = ReturnType<typeof useHotkeySettings>;

function HotkeyEditor({ model, available }: { model: HotkeyModel; available: typeof ACTIONS[number][] }) {
  const { profile, draft, busy, setDraft, run } = model;
  if (!profile || !draft) return null;
  const { current, revision } = profile;
  const write = (patch: { enabled?: boolean; osd?: boolean }) => {
    if (!current || !revision || busy) return;
    void run(() => applyHotkeys(revision, { ...current, ...patch }), "热键设置已回读", false);
  };
  const toggle = (scope: HotkeyScope, checked: boolean) => setDraft(previous => previous ? {
    ...previous, actions: ACTIONS.filter(action => action.scope === scope ? checked : previous.actions.includes(action.scope)).map(action => action.scope),
  } : previous);
  return <div className="ccc-detail-stack">
    <div className="ccc-keyboard-setting"><strong>接管原厂热键</strong><Switch label="启用原厂热键接管" checked={draft.enabled} disabled={busy || !revision}
      onChange={enabled => {
        if (!enabled && current?.enabled) write({ enabled });
        else setDraft({ ...draft, enabled });
      }} /></div>
    <div className="ccc-keyboard-setting"><strong>桌面提示</strong><Switch label="显示桌面提示" checked={current?.osd ?? false}
      disabled={busy || !current || !revision || current.enabled && (!profile.authorized || !profile.status.running)}
      onChange={osd => write({ osd })} /></div>
    <div className="ccc-hotkey-scopes" aria-label="热键动作">
      {available.map(action => {
        const cap = profile.facts.capabilities.find(cap => cap.scope === action.scope);
        return <div key={action.scope} className="ccc-hotkey-scope">
          <div className="ccc-rule-heading"><Icon name={action.icon} size={16} />
            <Checkbox label={action.title} checked={draft.actions.includes(action.scope)} disabled={busy}
              onChange={event => toggle(action.scope, event.target.checked)} /></div>
          {!cap?.identity && <p className="ccc-control-caption">暂不可用，请取消选择。</p>}
        </div>;
      })}
    </div>
    {draft.enabled && draft.actions.length === 0 && <p className="ccc-control-caption">请选择要接管的动作。</p>}
  </div>;
}

function HotkeyChanges({ model }: { model: HotkeyModel }) {
  const { profile, draft, busy, run } = model;
  if (!profile || !draft) return null;
  const blocked = draft.enabled && (!!profile.facts.blockedReason || !draft.actions.length ||
    draft.actions.some(scope => !profile.facts.capabilities.some(cap => cap.scope === scope && cap.identity)));
  const changed = JSON.stringify(draft) !== JSON.stringify(profile.current);
  const canApply = !!profile.revision && !blocked && (changed || draft.enabled && (!profile.status.running || !profile.authorized));
  return <ConfigurationAction revision={JSON.stringify([profile.revision, draft])} label="原厂热键与屏显" applyLabel="保存热键接管范围" disabled={busy || !canApply}
    onConfirm={() => run(() => applyHotkeys(profile.revision!, draft), "热键软件策略已保存", true)}>
    <p>接管{draft.enabled ? "开启" : "关闭"} · 桌面提示{draft.osd ? "开启" : "关闭"}</p>
    <ul className="ccc-parameter-descriptions">{ACTIONS.filter(action => draft.actions.includes(action.scope)).map(action =>
      <li key={action.scope}><strong>{action.title}</strong><p>{action.rule}</p></li>)}</ul>
    {draft.actions.includes("powerMode") && profile.facts.modeCycle?.nodeChoices.some(choice => choice.action?.target.enabled === false) &&
      <p className="ccc-control-caption">所选模式循环包含独显关断。</p>}
    {draft.actions.includes("batteryLowAction") && <p className="ccc-control-caption">低电量时可能进入睡眠或休眠。</p>}
    {draft.actions.includes("panelHotkey") && <p className="ccc-control-caption" aria-label="屏幕按键接管预览">
      {profile.facts.panelHotkey?.current?.kind === "blankSaver" ? "按键启动空白屏保，正常输入退出。"
        : profile.facts.panelHotkey?.current?.kind === "pointerWake" ? "按键开始等待；键盘按下取消，鼠标活动发送一次唤醒请求。" : "当前条件不可用，请重新读取。"}
    </p>}
  </ConfigurationAction>;
}

export function HotkeySettings({ active, revision }: { active: boolean; revision: number }) {
  const model = useHotkeySettings(active, revision), { profile, draft, busy, error, run } = model;
  const selected = new Set([...(draft?.actions ?? []), ...(profile?.current?.actions ?? [])]);
  const available = ACTIONS.filter(action => draft?.actions.includes(action.scope) ||
    profile?.facts.capabilities.some(cap => cap.scope === action.scope && cap.identity));
  const unavailable = profile ? ACTIONS.filter(action => !available.includes(action)) : [];
  const status = !profile ? error ? "读取失败" : "读取中…" : profile.status.running ? "正在监听"
    : profile.current?.enabled ? "已停止监听" : null;
  const savedCount = profile?.current?.actions.length ?? 0;
  const conflictIssues = [profile?.facts.blockedReason, error];
  const conflictMessage = findOemConflict(conflictIssues);
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="原厂热键与屏显">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />原厂热键与屏显</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="热键状态与诊断" label="详情" wide>
          {error && <p className="ccc-control-error">{error}</p>}
          {profile?.recordError && <p className="ccc-control-error">{profile.recordError}</p>}
          <HotkeyCurrent profile={profile} busy={busy} run={run} screenSelected={selected.has("screenBrightness")} panelSelected={selected.has("panelHotkey")} />
          <HotkeyEventPreviews profile={profile} selected={selected} draftActions={draft?.actions ?? []} />
          {unavailable.length > 0 && <section aria-label="不可用的热键动作"><h3>不可用的热键动作</h3>
            {unavailable.map(action => <p key={action.scope}><strong>{action.title}</strong> · {
              profile?.facts.capabilities.find(cap => cap.scope === action.scope)?.reason ?? "此设备未提供"}</p>)}
          </section>}
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取热键" disabled={busy} onClick={model.reread} />
      </div>
    </div>
    <div className="ccc-keyboard-summary">
      <div className="ccc-module-summary" role="status">
        {status && <strong>{status}</strong>}
        {savedCount > 0 && <span className="ccc-control-caption">已保存 {savedCount} 项</span>}
        {profile?.current?.enabled && !profile.authorized && <span className="ccc-control-caption">需要重新确认接管</span>}
        {conflictMessage && <span className="ccc-control-error">{conflictMessage}</span>}
        {!conflictMessage && (error || profile?.facts.blockedReason || profile?.status.error || profile?.osdError) &&
          <span className="ccc-control-caption">查看热键诊断</span>}
      </div>
      <div className="ccc-actions">
        {profile && draft && <DetailsDialog title="配置热键与屏显" label="配置热键" icon="edit" editor wide>
          <HotkeyEditor model={model} available={available} />
        </DetailsDialog>}
        {profile?.recordError && <ConfirmButton icon="delete" key={profile.resetRevision} label="清除不可用热键策略"
          confirmLabel="清除并停止监听" busy={busy} disabled={!profile.resetRevision}
          onConfirm={() => run(() => resetInvalidHotkeys(profile.resetRevision!), "不可用策略已清除，热键监听已停止", true)}>
          <p>保存关闭且未选择范围的策略，设备保持当前状态。</p>
        </ConfirmButton>}
      </div>
    </div>
    <OemConflictAction issues={conflictIssues} disabled={busy} onClosed={model.reread} />
    <HotkeyChanges model={model} />
    {profile && <HotkeyRecovery profile={profile} busy={busy} run={run} />}
  </Surface>;
}
