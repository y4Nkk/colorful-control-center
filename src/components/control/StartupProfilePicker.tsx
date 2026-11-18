import { Surface } from "../Surface";
import { Icon } from "../Icon";
import { Button } from "../Button";
import { ProfileModeAuthorization } from "../profiles/ProfileModeAuthorization";
import { Select } from "../Select";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { ProfilePreview } from "../profiles/ProfilePreview";
import { useStartupProfile } from "./useStartupProfile";

export function StartupProfilePicker({ active }: { active: boolean }) {
  const { profiles, selected, saved, selection, unchanged, error, lastRun, busy, ready, setSelected, apply, reread } = useStartupProfile(active);
  const preview = selection?.file ?? null;
  const { modeTarget = null, modeError = null, displayReview = null, displayError = null,
    gpuPowerReview = null, gpuPowerError = null, graphicsModeReview = null, graphicsModeError = null } = selection ?? {};
  const settings = preview?.settings;
  return (
    <Surface as="section" variant="group" className="ccc-panel ccc-startup-profile" aria-label="启动档案">
      <div className="ccc-panel-head">
        <h2 className="ccc-panel-title"><Icon name="play" /><label htmlFor="startup-profile">启动档案</label></h2>
        {saved !== null && <span className="ccc-chip">已保存 · {saved || "不应用档案"}</span>}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取启动档案" title="重新读取启动档案" disabled={!ready || busy} onClick={reread} />
      </div>
      <div className="ccc-control-status">
        {lastRun && <span className="ccc-control-caption" role="status">{lastRun}</span>}
        {error && <span role="alert" className="ccc-control-error">{error}</span>}
        {[modeError, graphicsModeError, gpuPowerError, displayError].filter(Boolean).map(issue => <span role="alert" className="ccc-control-error" key={issue}>{issue}</span>)}
        {ready && saved === null && <span className="ccc-control-caption">当前保存的启动选择未知，请重新选择或确认取消。</span>}
      </div>
      <div className="ccc-actions ccc-startup-control">
        <Select id="startup-profile" label="启动档案" value={selected} disabled={!ready || busy} onChange={setSelected}
          options={[{ value: "", label: "不应用档案" }, ...profiles.map(p => ({ value: p.name, label: p.name }))]} />
      </div>
      <ConfigurationAction label="启动档案" revision={JSON.stringify([selected, saved, selection])}
        disabled={busy || !ready || unchanged || Boolean(selected && (!preview || modeError || gpuPowerError || graphicsModeError || displayError))} onConfirm={apply}>
        <p>{selected ? `启动时应用「${selected}」` : "启动时不再应用档案"}</p>
        <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
        {selected && settings && <DetailsDialog title="启动档案参数" wide>
          <ProfilePreview settings={settings} modeTarget={modeTarget} displayReview={displayReview} gpuPowerReview={gpuPowerReview} graphicsModeReview={graphicsModeReview} />
          {settings.oemPowerMode != null && <p>启动时保存插电偏好并应用模式的固件节能联动。</p>}
        </DetailsDialog>}
      </ConfigurationAction>
    </Surface>
  );
}
