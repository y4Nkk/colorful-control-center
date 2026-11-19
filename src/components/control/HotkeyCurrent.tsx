import { previewOsd, stopHotkeys, type HotkeyProfile } from "../../api/client";
import type { useHotkeySettings } from "./useHotkeySettings";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ScreenBrightnessFacts } from "./ScreenBrightnessFacts";
import { PanelHotkeyFacts } from "./PanelHotkeyFacts";
import { summary } from "./hotkey-actions";

type Props = { profile: HotkeyProfile | null; busy: boolean; run: ReturnType<typeof useHotkeySettings>["run"]; screenSelected: boolean; panelSelected: boolean };
export function HotkeyCurrent({ profile, busy, run, screenSelected, panelSelected }: Props) {
  const screen = profile?.facts.screenBrightness, panel = profile?.facts.panelHotkey;
  return <Surface variant="inset" className="ccc-hotkey-section">
    <strong>{!profile ? "监听状态未知" : profile.status.running ? "正在监听原厂热键" : "热键监听已停止"}</strong>
    {profile && <>
      <span className="ccc-control-caption">已保存 · {profile.current ? `${profile.current.enabled ? "启用" : "关闭"} · ${profile.current.actions.length} 项` : "未知"}</span>
      {profile.current?.enabled && !profile.authorized && <p className="ccc-control-caption">授权条件已变化，请重新确认。</p>}
      {profile.facts.blockedReason && <p className="ccc-control-caption">{profile.facts.blockedReason}</p>}
      {profile.status.error && <p className="ccc-control-error" role="alert">{profile.status.error}</p>}
      {profile.status.lastResult && <p className="ccc-control-caption" role="status">{profile.status.lastResult}</p>}
      {profile.osdError && <p className="ccc-control-error" role="alert">屏显：{profile.osdError}</p>}
    </>}
    <div className="ccc-actions">
      <Button icon="pause" variant="secondary" disabled={busy} onClick={() => run(stopHotkeys, "本次热键监听已停止", false)}>临时停止监听</Button>
      <Button icon="search" variant="ghost" disabled={busy} onClick={() => run(previewOsd, "已请求屏显预览", false)}>预览桌面屏显</Button>
    </div>
    {screen && (screenSelected || screen.initial.length > 0) && <ScreenBrightnessFacts profile={screen} busy={busy} run={run} />}
    {panel && (panelSelected || panel.session !== null) && <PanelHotkeyFacts profile={panel} busy={busy} run={run} />}
    <DetailsDialog title="监听与恢复说明">
      {profile?.current && <p>当前范围：{summary(profile.current)}</p>}
      <p>临时停止保留已保存策略，重新打开应用可恢复监听；硬件初始状态在对应模块恢复。</p>
      {profile?.initial && <p>初始策略：{profile.initial.enabled ? "启用" : "关闭"} · {summary(profile.initial)}</p>}
    </DetailsDialog>
  </Surface>;
}
