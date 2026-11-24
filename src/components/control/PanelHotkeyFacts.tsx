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
        <strong>按键的效果</strong>
        {current ? <>
          <div><strong className="ccc-panel-hotkey-value">{names[current.kind]}</strong>
            <p className="ccc-control-caption">已确认 {current.inventory.monitors.length} 块屏幕</p></div>
          <span className="ccc-control-caption">{current.kind === "blankSaver" ? "任意输入退出" : "鼠标唤醒 · 键盘取消"}</span>
          <DetailsDialog title="屏幕按键说明"><p className="ccc-control-caption">{current.kind === "blankSaver"
            ? "这块屏幕符合原厂的 OLED 条件。按下按键后会显示 Windows 的空白屏幕保护，按任意键或动一下鼠标即可退出；屏幕并没有真正断电。"
            : "按下按键后开始等待：敲键盘会直接结束等待；动鼠标则会向屏幕发送一次唤醒请求。等待期间的按键和鼠标操作照常有效。"}</p></DetailsDialog>
          {current.inventory.unresolved.length > 0 && <p className="ccc-control-caption">还有 {current.inventory.unresolved.length} 块屏幕无法确认型号；已确认是 OLED 的屏幕仍按上面的规则处理。</p>}
        </> : <p className="ccc-control-caption">本次适用行为不可用，请重新读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-panel-hotkey-part">
        <strong>本次接管</strong>
        <div><strong className="ccc-panel-hotkey-value">{session ? session.preparing ? "正在准备" : session.error ? "等待结束重试" : "进行中" : "未运行"}</strong>
          <p className="ccc-control-caption">{session ? `本次行为：${names[session.kind]}` : "尚未开始"}</p></div>
        <span className="ccc-control-caption">结束时会停止监听，已保存的热键设置保持不变</span>
        {session?.error && <p className="ccc-control-error">{session.error}</p>}
        {session && <ConfirmButton key={profile.restoreRevision ?? session.id} variant="secondary"
          label="结束本次屏幕接管" confirmLabel="确认结束本次接管并停止监听？" busy={busy} disabled={!profile.restoreRevision}
          onConfirm={() => run(() => endPanelHotkey(profile.restoreRevision!), "本软件屏幕会话已结束", false)} />}
      </Surface>
    </div>
  </section>;
}
