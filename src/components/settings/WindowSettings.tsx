import { Icon } from "../Icon";
import { Surface } from "../Surface";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { AutostartSetting } from "./AutostartSetting";
import { Switch } from "../Switch";
import { PreferenceCards, SettingRow } from "./PreferenceCards";
import { ClosePreview } from "./SettingsPreviews";
import { SegmentedControl } from "../SegmentedControl";
import { ShortcutSetting } from "./ShortcutSetting";

export function WindowSettings({ model, active }: { model: PreferencesModel; active: boolean }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <Surface as="section" variant="group" className="ccc-settings-group" aria-label="窗口与托盘">
    <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title"><Icon name="window" />窗口与托盘</h2></header>
    <PreferenceCards label="点击关闭按钮时" value={settings.closeBehavior} busy={busy}
      options={[{ value: "hideToTray", label: "最小化到托盘" },
        { value: "exit", label: "退出应用" }]}
      preview={behavior => <ClosePreview behavior={behavior} />} onChange={closeBehavior => void patch({ closeBehavior })} />
    <SettingRow label="窗口始终置顶"><Switch label="窗口始终置顶" checked={settings.alwaysOnTop} disabled={busy}
      onChange={alwaysOnTop => void patch({ alwaysOnTop })} /></SettingRow>
    <AutostartSetting active={active} busy={busy} />
    <SettingRow label="启动时最小化到托盘"><Switch label="启动时最小化到托盘" checked={settings.startMinimized} disabled={busy}
      onChange={startMinimized => void patch({ startMinimized })} /></SettingRow>
    <SettingRow label="启动页面"><SegmentedControl label="启动页面" value={settings.startupPage} disabled={busy}
      options={[{ value: "dashboard", label: "概览" },
        { value: "lastVisited", label: "上次访问页面" }]}
      onChange={value => void patch({ startupPage: value === "lastVisited" ? value : "dashboard" })} /></SettingRow>
    <ShortcutSetting value={settings.globalShortcut} busy={busy || !active} active={active}
      onChange={globalShortcut => patch({ globalShortcut })} />
  </Surface>;
}
