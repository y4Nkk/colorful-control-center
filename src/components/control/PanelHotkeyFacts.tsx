import { endPanelHotkey, type PanelHotkeyKind, type PanelHotkeyProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";

type Props = { profile: PanelHotkeyProfile; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
const names: Record<PanelHotkeyKind, string> = { blankSaver: "空白屏保", pointerWake: "一次性输入唤醒" };
export function PanelHotkeyFacts({ profile, busy, run }: Props) {
  const { current, session } = profile;
  return <section className="ccc-panel-hotkey-facts" aria-label="原厂屏幕按键事实">
    <strong>原厂屏幕按键</strong>
    <div className="ccc-panel-hotkey-columns">
      <Surface variant="inset" className="ccc-panel-hotkey-part">
        <strong>适用行为</strong>
        {current ? <>
          <div><strong className="ccc-panel-hotkey-value">{names[current.kind]}</strong>
            <p className="ccc-control-caption">已确认 {current.inventory.monitors.length} 台面板</p></div>
          <span className="ccc-chip">{current.kind === "blankSaver" ? "任意输入退出" : "鼠标唤醒 · 键盘取消"}</span>
          <DetailsDialog title="屏幕按键说明"><p className="ccc-control-caption">{current.kind === "blankSaver"
            ? "已匹配原厂 OLED 条件。按键启动 Windows 空白屏保，正常输入退出；不表示面板已断电。"
            : "首个键盘按下结束等待；首个鼠标活动发送一次屏幕唤醒请求。输入照常传递。"}</p></DetailsDialog>
          {current.inventory.unresolved.length > 0 && <p className="ccc-control-caption">另有 {current.inventory.unresolved.length} 个面板身份未确认；已确认 OLED 的适用条件保留。</p>}
        </> : <p className="ccc-control-caption">本次适用行为不可用，请重新读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-panel-hotkey-part">
        <strong>本软件会话</strong>
        <div><strong className="ccc-panel-hotkey-value">{session ? session.preparing ? "正在准备" : session.error ? "等待结束重试" : "进行中" : "未运行"}</strong>
          <p className="ccc-control-caption">{session ? `会话行为：${names[session.kind]}` : "尚未创建会话"}</p></div>
        <span className="ccc-chip">结束时停止监听 · 保留策略</span>
        {session?.error && <p className="ccc-control-error">{session.error}</p>}
        {session && <ConfirmButton key={profile.restoreRevision ?? session.id} variant="secondary"
          label="结束本软件屏幕会话" confirmLabel="确认结束会话并停止监听？" busy={busy} disabled={!profile.restoreRevision}
          onConfirm={() => run(() => endPanelHotkey(profile.restoreRevision!), "本软件屏幕会话已结束", false)} />}
      </Surface>
    </div>
  </section>;
}
