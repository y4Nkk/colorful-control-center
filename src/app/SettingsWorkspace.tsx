/** 图形化应用设置；各组拥有实际配置的读取、保存与反馈。 */
import { Button } from "../components/Button";
import { AppearanceSettings } from "../components/settings/AppearanceSettings";
import { WindowSettings } from "../components/settings/WindowSettings";
import { RuntimeSettings } from "../components/settings/RuntimeSettings";
import { AboutSettings } from "../components/settings/AboutSettings";
import type { PreferencesModel } from "./useAppPreferences";
import { DetailsDialog } from "../components/DetailsDialog";
import { Loading } from "../components/Loading";

export function SettingsWorkspace({ active, model }: { active: boolean; model: PreferencesModel }) {
  return (
    <div className="ccc-module-grid" data-module-layout="settings" aria-label="应用设置" aria-busy={model.busy}>
      <h2 className="ccc-settings-title">设置</h2>
      {(model.error || model.snapshot?.desktopError) && <PreferencesFeedback model={model} />}
      {!model.snapshot ? (model.busy ? <Loading label="正在读取软件偏好…" /> :
        <p role="status" className="ccc-settings-detail">软件偏好不可用，原配置已保留。</p>) : <>
        <AppearanceSettings model={model} />
        <WindowSettings active={active} model={model} />
        <RuntimeSettings model={model} />
      </>}
      <AboutSettings />
    </div>
  );
}

function PreferencesFeedback({ model }: { model: PreferencesModel }) {
  return <div className="ccc-settings-feedback">
    <p className="ccc-settings-error" role="alert">部分偏好未能加载或保存。</p>
    <div className="ccc-setting-control">
      <DetailsDialog title="软件偏好诊断" label="查看原因">
        <p>如果设置读取失败，可以关闭此弹窗，点「重新读取」再试一次。</p>
        <p>{model.error ?? model.snapshot?.desktopError}</p>
      </DetailsDialog>
      <Button icon="refresh" variant="secondary" size="compact" disabled={model.busy} onClick={() => void model.reload()}>重新读取</Button>
    </div>
  </div>;
}
