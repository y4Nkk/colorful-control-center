import type { PreferencesModel } from "../../app/useAppPreferences";
import { pageTitle } from "../../app/navigation";
import { AutostartSetting } from "./AutostartSetting";
import { Switch } from "../Switch";
import { PreferenceCards } from "./PreferenceCards";
import { SettingRow, SettingsSection } from "./SettingsSection";
import { ClosePreview } from "./SettingsPreviews";
import { SegmentedControl } from "../SegmentedControl";
import { ShortcutSetting } from "./ShortcutSetting";
import { toast } from "../use-toast";

const ON_TOP_LABEL = "窗口始终置顶";
const STARTUP_VISIBILITY_LABEL = "启动时显示";
const STARTUP_PAGE_LABEL = "启动页面";

export function WindowSettings({ model, active }: { model: PreferencesModel; active: boolean }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <>
    <SettingsSection title="窗口与托盘" icon="window">
      <PreferenceCards label="点击关闭按钮时" value={settings.closeBehavior} busy={busy}
        options={[{ value: "hideToTray", label: "最小化到托盘" },
          { value: "exit", label: "退出应用" }]}
        preview={behavior => <ClosePreview behavior={behavior} />} onChange={closeBehavior => void patch({ closeBehavior })} />
      <SettingRow label={ON_TOP_LABEL}><Switch label={ON_TOP_LABEL} checked={settings.alwaysOnTop} disabled={busy}
        onChange={alwaysOnTop => void patch({ alwaysOnTop })} /></SettingRow>
      <ShortcutSetting value={settings.globalShortcut} busy={busy || !active} active={active}
        onChange={globalShortcut => patch({ globalShortcut })} />
    </SettingsSection>
    <SettingsSection title="启动">
      <AutostartSetting active={active} busy={busy} />
      <SettingRow label={STARTUP_VISIBILITY_LABEL} detail="适用于登录自启和手动启动。">
        <SegmentedControl label={STARTUP_VISIBILITY_LABEL} value={settings.startMinimized ? "tray" : "window"} disabled={busy}
          options={[{ value: "window", label: "显示主界面" }, { value: "tray", label: "最小化到托盘" }]}
          onChange={async value => {
            if (await patch({ startMinimized: value === "tray" })) toast({ kind: "success", title: "已保存启动显示方式" });
          }} />
      </SettingRow>
      <SettingRow label={STARTUP_PAGE_LABEL}><SegmentedControl label={STARTUP_PAGE_LABEL} value={settings.startupPage} disabled={busy}
        options={[{ value: "dashboard", label: pageTitle("dashboard") },
          { value: "lastVisited", label: "上次访问页面" }]}
        onChange={value => void patch({ startupPage: value === "lastVisited" ? value : "dashboard" })} /></SettingRow>
    </SettingsSection>
  </>;
}
