import { Button } from "../Button";
import { ProfileModeAuthorization } from "../profiles/ProfileModeAuthorization";
import { Select } from "../Select";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { ProfilePreview } from "../profiles/ProfilePreview";
import { useStartupProfile } from "./useStartupProfile";

export function StartupProfilePicker({ active }: { active: boolean }) {
  const { profiles, selected, saved, selection, unchanged, error, lastRun, busy, ready, choose, apply, reread } = useStartupProfile(active);
  const preview = selection?.file ?? null;
  const { modeTarget = null, modeError = null, displayReview = null, displayError = null,
    gpuPowerReview = null, gpuPowerError = null, graphicsModeReview = null, graphicsModeError = null } = selection ?? {};
  const settings = preview?.settings;
  return (
    <section aria-label="启动档案">
      <div className="ccc-power-setting">
        <div>
          <div className="ccc-power-setting-label">
            <label htmlFor="startup-profile">启动档案</label>
            <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取启动档案" title="重新读取启动档案" disabled={!ready || busy} onClick={reread} />
          </div>
          {saved !== null && selected !== saved && <p className="ccc-control-caption">已保存 · {saved || "不应用档案"}</p>}
          {lastRun && <p className="ccc-control-caption" role="status">{lastRun}</p>}
        </div>
        <div className="ccc-power-setting-picker">
          <Select id="startup-profile" label="启动档案" size="compact" value={!selected && saved === null ? "\u0000" : selected}
            placeholder="选择启动档案" disabled={!ready || busy} onChange={choose}
            options={[{ value: "", label: "不应用档案" }, ...profiles.map(p => ({ value: p.name, label: p.name }))]} />
        </div>
      </div>
      {error && <p role="alert" className="ccc-control-error">{error}</p>}
      {[modeError, graphicsModeError, gpuPowerError, displayError].filter(Boolean).map(issue => <p role="alert" className="ccc-control-error" key={issue}>{issue}</p>)}
      <OemConflictAction issues={[error, modeError, graphicsModeError, gpuPowerError, displayError, lastRun]} disabled={busy} onClosed={reread} />
      {ready && saved === null && <p className="ccc-control-caption">当前保存的启动选择未知。</p>}
      <ConfigurationAction label="启动档案" applyLabel="授权启动应用档案" revision={JSON.stringify([selected, saved, selection])}
        disabled={busy || !ready || unchanged || !selected || !preview || Boolean(modeError || gpuPowerError || graphicsModeError || displayError)} onConfirm={apply}>
        <p>启动时应用「{selected}」</p>
        <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
        {selected && settings && <DetailsDialog title="启动档案参数" wide>
          <ProfilePreview settings={settings} modeTarget={modeTarget} displayReview={displayReview} gpuPowerReview={gpuPowerReview} graphicsModeReview={graphicsModeReview} />
          {settings.oemPowerMode != null && <p>启动时保存插电偏好并应用模式的固件节能联动。</p>}
        </DetailsDialog>}
      </ConfigurationAction>
    </section>
  );
}
