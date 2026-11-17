import { Icon } from "../Icon";
import { Surface } from "../Surface";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { PreferenceCards, SettingRow } from "./PreferenceCards";
import { FontPicker } from "./FontPicker";
import { ThemePreview, MotionPreview } from "./SettingsPreviews";

export function AppearanceSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <Surface as="section" variant="group" className="ccc-settings-group" aria-label="外观">
    <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title"><Icon name="appearance" />外观</h2></header>
    <PreferenceCards label="界面主题" value={settings.theme} busy={busy}
      options={[{ value: "light", label: "浅色" },
        { value: "system", label: "跟随系统" },
        { value: "dark", label: "深色" }]}
      preview={theme => <ThemePreview theme={theme} />} onChange={theme => void patch({ theme })} />
    <SettingRow label="界面字体"><FontPicker value={settings.interfaceFont} busy={busy}
      onChange={interfaceFont => patch({ interfaceFont })} /></SettingRow>
    <PreferenceCards label="主界面缩放" value={settings.interfaceScale} busy={busy}
      options={[{ value: 90, label: "90%" },
        { value: 100, label: "100%" },
        { value: 110, label: "110%" },
        { value: 125, label: "125%" }]}
      preview={scale => <ThemePreview theme="light" scale={scale} />}
      onChange={interfaceScale => void patch({ interfaceScale })} />
    <PreferenceCards label="动态效果" value={settings.motion} busy={busy}
      options={[{ value: "system", label: "跟随系统" },
        { value: "reduce", label: "减少动态效果" }]}
      preview={mode => <MotionPreview mode={mode} />} onChange={motion => void patch({ motion })} />
  </Surface>;
}
