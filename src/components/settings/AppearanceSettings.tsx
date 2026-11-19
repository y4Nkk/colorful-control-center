import { Icon } from "../Icon";
import { Surface } from "../Surface";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { PreferenceCards, SettingRow } from "./PreferenceCards";
import { FontPicker } from "./FontPicker";
import { ThemePreview } from "./SettingsPreviews";
import { SegmentedControl } from "../SegmentedControl";
import { Switch } from "../Switch";

const SCALES = [90, 100, 110, 125] as const;
export function AppearanceSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <Surface as="section" variant="group" className="ccc-settings-group" aria-label="外观">
    <header className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="appearance" />外观</h2></header>
    <PreferenceCards label="界面主题" value={settings.theme} busy={busy}
      options={[{ value: "light", label: "浅色" },
        { value: "system", label: "跟随系统" },
        { value: "dark", label: "深色" }]}
      preview={theme => <ThemePreview theme={theme} />} onChange={theme => void patch({ theme })} />
    <SettingRow label="界面字体"><FontPicker value={settings.interfaceFont} busy={busy}
      onChange={interfaceFont => patch({ interfaceFont })} /></SettingRow>
    <SettingRow label="主界面缩放"><SegmentedControl label="主界面缩放" value={String(settings.interfaceScale)} disabled={busy}
      options={SCALES.map(scale => ({ value: String(scale), label: `${scale}%` }))}
      onChange={value => { const interfaceScale = SCALES.find(scale => String(scale) === value); if (interfaceScale) void patch({ interfaceScale }); }} />
    </SettingRow>
    <SettingRow label="减少动态效果"><Switch label="减少动态效果" checked={settings.motion === "reduce"} disabled={busy}
      onChange={reduce => void patch({ motion: reduce ? "reduce" : "system" })} /></SettingRow>
  </Surface>;
}
