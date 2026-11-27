import type { PreferencesModel } from "../../app/useAppPreferences";
import { PreferenceCards } from "./PreferenceCards";
import { SettingRow, SettingsSection } from "./SettingsSection";
import { FontPicker } from "./FontPicker";
import { ThemePreview } from "./SettingsPreviews";
import { SegmentedControl } from "../SegmentedControl";
import { Switch } from "../Switch";

const SCALES = [90, 100, 110, 125] as const;
const SCALE_LABEL = "主界面缩放";
const MOTION_LABEL = "减少动态效果";

export function AppearanceSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <SettingsSection title="外观" icon="appearance">
    <PreferenceCards label="界面主题" value={settings.theme} busy={busy}
      options={[{ value: "light", label: "浅色" },
        { value: "system", label: "跟随系统" },
        { value: "dark", label: "深色" }]}
      preview={theme => <ThemePreview theme={theme} />} onChange={theme => void patch({ theme })} />
    <SettingRow label="界面字体">
      <div className="ccc-settings-select"><FontPicker value={settings.interfaceFont} busy={busy}
        onChange={interfaceFont => patch({ interfaceFont })} /></div>
    </SettingRow>
    <SettingRow label={SCALE_LABEL}>
      <div className="ccc-settings-scale">
        <SegmentedControl label={SCALE_LABEL} value={String(settings.interfaceScale)} disabled={busy} balancedWrap
          options={SCALES.map(scale => ({ value: String(scale), label: `${scale}%` }))}
          onChange={value => { const interfaceScale = SCALES.find(scale => String(scale) === value); if (interfaceScale) void patch({ interfaceScale }); }} />
      </div>
    </SettingRow>
    <SettingRow label={MOTION_LABEL} detail="关闭后跟随系统动画设置。"><Switch label={MOTION_LABEL} checked={settings.motion === "reduce"} disabled={busy}
      onChange={reduce => void patch({ motion: reduce ? "reduce" : "system" })} /></SettingRow>
  </SettingsSection>;
}
