import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { ConfirmButton } from "../ConfirmButton";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { Switch } from "../Switch";
import { SettingRow } from "./PreferenceCards";
import { Select } from "../Select";

const LEVELS = [
  { value: "debug", label: "调试" }, { value: "info", label: "信息" }, { value: "warn", label: "警告" },
  { value: "error", label: "错误" }, { value: "silent", label: "静默" },
] as const;

export function RuntimeSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch, restart } = model;
  if (!snapshot) return null;
  return <>
    <Surface as="section" variant="group" className="ccc-settings-group" aria-label="运行与诊断">
      <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title"><Icon name="performance" />运行与诊断</h2>
        <DetailsDialog title="运行日志" label="日志设置" icon="logs">
          <SettingRow label="日志级别"><Select label="日志级别" value={snapshot.settings.runtimeLogLevel}
            disabled={busy} options={LEVELS}
            onChange={value => { const level = LEVELS.find(option => option.value === value); if (level) void patch({ runtimeLogLevel: level.value }); }} /></SettingRow>
        </DetailsDialog>
      </header>
      <SettingRow label="硬件加速">
        <Switch label="硬件加速" checked={snapshot.settings.hardwareAcceleration} disabled={busy}
          onChange={hardwareAcceleration => void patch({ hardwareAcceleration })} />
      </SettingRow>
      {snapshot.restartRequired && <div className="ccc-settings-feedback">
        <p className="ccc-settings-detail" role="status">重启应用后生效</p>
        <ConfirmButton label="立即重启" confirmLabel="重启应用" disabled={busy} onConfirm={restart}>
          <p>重启后应用硬件加速设置。尚未应用的草稿会丢失。</p>
        </ConfirmButton>
      </div>}
    </Surface>
  </>;
}
