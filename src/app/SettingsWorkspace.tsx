/** 图形化应用设置；各组拥有实际配置的读取、保存与反馈。 */
import { Button } from "../components/Button";
import { AppearanceSettings } from "../components/settings/AppearanceSettings";
import { WindowSettings } from "../components/settings/WindowSettings";
import { RuntimeSettings } from "../components/settings/RuntimeSettings";
import { AboutSettings } from "../components/settings/AboutSettings";
import type { PreferencesModel } from "./useAppPreferences";

export function SettingsWorkspace({ active, model }: { active: boolean; model: PreferencesModel }) {
  return (
    <div className="ccc-settings" aria-label="应用设置" aria-busy={model.busy}>
      {(model.error || model.snapshot?.desktopError) && <div className="ccc-settings-feedback">
        <p className="ccc-settings-error" role="alert">{model.error ?? model.snapshot?.desktopError}</p>
        <Button icon="refresh" type="button" disabled={model.busy} onClick={() => void model.reload()}>重新读取软件偏好</Button>
      </div>}
      {!model.snapshot ? <p role="status" className="ccc-settings-detail">{model.busy ? "正在读取软件偏好…" : "软件偏好不可用，原配置已保留。"}</p> : <>
        <AppearanceSettings model={model} />
        <WindowSettings active={active} model={model} />
        <RuntimeSettings model={model} />
      </>}
      <AboutSettings active={active} />
    </div>
  );
}
