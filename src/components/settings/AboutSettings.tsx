import { BrandLogo } from "../BrandLogo";
import { Loading } from "../Loading";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { useApplicationUpdate } from "../../app/useApplicationUpdate";
import { ChangelogHistory } from "./ChangelogHistory";
import { SettingRow, SettingsSection } from "./SettingsSection";

export function AboutSettings() {
  const { version, retryVersion, state, checkOnEntry, activate } = useApplicationUpdate();
  const checking = state.phase === "checking";
  const currentVersion = version.status === "ready" ? version.version : undefined;
  return (
    <SettingsSection title="关于" icon="info">
      <div className="ccc-settings-item ccc-setting-row">
        <div className="ccc-about-product">
          <BrandLogo className="ccc-about-logo" />
          <div className="ccc-about-copy"><h4>Colorful Control Center</h4><p>社区项目 · 非官方</p></div>
        </div>
        <div className="ccc-setting-control">
          {version.status === "loading" ? <Loading label="正在读取版本…" /> :
            <span className="ccc-settings-status ccc-mono" role="status">{currentVersion ? `v${currentVersion}` : "版本未知"}</span>}
        </div>
      </div>
      {version.status === "error" && <div className="ccc-settings-item ccc-settings-feedback">
        <p className="ccc-settings-error" role="alert">版本读取失败：{version.message}</p>
        <Button icon="refresh" variant="secondary" size="compact" onClick={retryVersion}>重新读取版本</Button>
      </div>}
      <SettingRow label="应用更新">
        <Button icon={checking ? undefined : "refresh"} variant="secondary" size="compact" aria-haspopup="dialog"
          aria-disabled={checking || undefined} aria-busy={checking || undefined} onClick={() => activate("about")}>
          {checking ? <Loading embedded label="正在检查更新" /> : checkOnEntry ? "检查更新" : "查看更新状态"}
        </Button>
        <DetailsDialog title="更新日志" icon="logs">
          <ChangelogHistory appVersion={currentVersion} />
        </DetailsDialog>
      </SettingRow>
    </SettingsSection>
  );
}
