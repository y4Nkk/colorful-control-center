import { ConfirmButton } from "../ConfirmButton";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { Switch } from "../Switch";
import { SettingRow, SettingsSection } from "./SettingsSection";
import { Select } from "../Select";

const ACCELERATION_LABEL = "硬件加速";
const OEM_CLOSE_LABEL = "自动关闭原厂软件";
const LOG_LEVEL_LABEL = "日志级别";

const LEVELS = [
  { value: "debug", label: "调试" }, { value: "info", label: "信息" }, { value: "warn", label: "警告" },
  { value: "error", label: "错误" }, { value: "silent", label: "静默" },
] as const;

export function RuntimeSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch, restart } = model;
  if (!snapshot) return null;
  const settings = snapshot.settings;
  return <SettingsSection title="运行与诊断" icon="performance">
    <SettingRow label={ACCELERATION_LABEL}>
      <Switch label={ACCELERATION_LABEL} checked={settings.hardwareAcceleration} disabled={busy}
        onChange={hardwareAcceleration => void patch({ hardwareAcceleration })} />
    </SettingRow>
    {snapshot.restartRequired && <div className="ccc-settings-item ccc-settings-feedback">
      <p className="ccc-settings-detail" role="status">重启应用后生效</p>
      <ConfirmButton label="立即重启" confirmLabel="重启应用" disabled={busy} onConfirm={restart}>
        <p>重启后应用硬件加速设置。尚未应用的草稿会丢失。</p>
      </ConfirmButton>
    </div>}
    <SettingRow label={OEM_CLOSE_LABEL}
      detail="启动和运行时关闭已核验的原厂控制中心、Flexikey 与 FnKey，最小化到托盘时也生效。关闭 FnKey 会停用原厂热键和屏显。">
      <Switch label={OEM_CLOSE_LABEL} checked={settings.closeOemOnStartup} disabled={busy}
        onChange={closeOemOnStartup => void patch({ closeOemOnStartup })} />
    </SettingRow>
    <SettingRow label={LOG_LEVEL_LABEL}>
      <div className="ccc-settings-select">
        <Select label={LOG_LEVEL_LABEL} value={settings.runtimeLogLevel} disabled={busy} options={LEVELS}
          onChange={value => {
            const level = LEVELS.find(option => option.value === value);
            if (level) void patch({ runtimeLogLevel: level.value });
          }} />
      </div>
    </SettingRow>
  </SettingsSection>;
}
