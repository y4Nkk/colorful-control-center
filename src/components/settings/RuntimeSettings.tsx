import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import type { PreferencesModel } from "../../app/useAppPreferences";
import { Switch } from "../Switch";
import { PreferenceCards, SettingRow } from "./PreferenceCards";
import { LogPreview } from "./SettingsPreviews";

export function RuntimeSettings({ model }: { model: PreferencesModel }) {
  const { snapshot, busy, patch, restart } = model;
  if (!snapshot) return null;
  return <>
    <Surface as="section" variant="group" className="ccc-settings-group" aria-label="运行日志">
      <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title"><Icon name="logs" />运行日志</h2></header>
      <PreferenceCards label="日志级别"
        value={snapshot.settings.runtimeLogLevel} busy={busy}
        options={[{ value: "debug", label: "调试" },
          { value: "info", label: "信息" },
          { value: "warn", label: "警告" },
          { value: "error", label: "错误" },
          { value: "silent", label: "静默" }]}
        preview={level => <LogPreview level={level} />} onChange={runtimeLogLevel => void patch({ runtimeLogLevel })} />
    </Surface>
    <Surface as="section" variant="group" className="ccc-settings-group" aria-label="渲染性能">
      <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title"><Icon name="performance" />渲染性能</h2></header>
      <SettingRow label="硬件加速" detail="重启应用后生效">
        <Switch label="硬件加速" checked={snapshot.settings.hardwareAcceleration} disabled={busy}
          onChange={hardwareAcceleration => void patch({ hardwareAcceleration })} />
      </SettingRow>
      {snapshot.restartRequired && <div className="ccc-settings-feedback"><p className="ccc-settings-detail" role="status">重启应用后生效</p>
        <Button type="button" disabled={busy} onClick={() => void restart()}>立即重启</Button></div>}
    </Surface>
  </>;
}
