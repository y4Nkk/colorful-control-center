import { Icon } from "../Icon";
import { Surface } from "../Surface";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { pageTitle } from "../../app/navigation";
import { AutostartSetting } from "./AutostartSetting";
import { Switch } from "../Switch";
import { PreferenceCards, SettingRow } from "./PreferenceCards";
import { ClosePreview } from "./SettingsPreviews";
import { SegmentedControl } from "../SegmentedControl";
import { ShortcutSetting } from "./ShortcutSetting";
import { toast } from "../use-toast";

export function WindowSettings({ model, active }: { model: PreferencesModel; active: boolean }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <Surface as="section" variant="group" className="ccc-settings-group" aria-label="窗口与托盘">
    <header className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="window" />窗口与托盘</h2></header>
    <PreferenceCards label="点击关闭按钮时" value={settings.closeBehavior} busy={busy}
      options={[{ value: "hideToTray", label: "最小化到托盘" },
        { value: "exit", label: "退出应用" }]}
      preview={behavior => <ClosePreview behavior={behavior} />} onChange={closeBehavior => void patch({ closeBehavior })} />
    <SettingRow label="窗口始终置顶"><Switch label="窗口始终置顶" checked={settings.alwaysOnTop} disabled={busy}
      onChange={alwaysOnTop => void patch({ alwaysOnTop })} /></SettingRow>
    <AutostartSetting active={active} busy={busy} />
    <SettingRow label="启动时显示" detail="适用于登录自启和手动启动。">
      <SegmentedControl label="启动时显示" className="ccc-startup-display" value={settings.startMinimized ? "tray" : "window"} disabled={busy}
        options={[{ value: "window", label: "显示主界面" }, { value: "tray", label: "最小化到托盘" }]}
        onChange={async value => {
          if (await patch({ startMinimized: value === "tray" })) toast({ kind: "success", title: "已保存启动显示方式" });
        }} />
    </SettingRow>
    <SettingRow label="启动时关闭原厂软件" detail="结束已核验的 FnKey。原厂热键和屏显会随它退出。">
      <Switch label="启动时关闭原厂软件" checked={settings.closeOemOnStartup} disabled={busy}
        onChange={closeOemOnStartup => void patch({ closeOemOnStartup })} />
    </SettingRow>
    <SettingRow label="启动页面"><SegmentedControl label="启动页面" value={settings.startupPage} disabled={busy}
      options={[{ value: "dashboard", label: pageTitle("dashboard") },
        { value: "lastVisited", label: "上次访问页面" }]}
      onChange={value => void patch({ startupPage: value === "lastVisited" ? value : "dashboard" })} /></SettingRow>
    <ShortcutSetting value={settings.globalShortcut} busy={busy || !active} active={active}
      onChange={globalShortcut => patch({ globalShortcut })} />
  </Surface>;
}
