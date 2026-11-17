import { Surface } from "../Surface";
import { Icon } from "../Icon";
import { Button } from "../Button";
import { ProfileModeAuthorization } from "../profiles/ProfileModeAuthorization";
import { Select } from "../Select";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
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
      <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
      {settings && <Collapsible summary="查看启动档案参数">
        <ProfilePreview settings={settings} modeTarget={modeTarget} displayReview={displayReview} gpuPowerReview={gpuPowerReview} graphicsModeReview={graphicsModeReview} />
        {settings.oemPowerMode != null && <p className="ccc-control-caption">启动时选择该模式也会保存为插电偏好并应用固件节能联动，电池供电时同样如此；启动时将重新检查能力及状态。</p>}
      </Collapsible>}
      <div className="ccc-actions ccc-startup-control">
        <Select id="startup-profile" label="启动档案" value={selected} disabled={!ready || busy} onChange={setSelected}
          options={[{ value: "", label: "不应用档案" }, ...profiles.map(p => ({ value: p.name, label: p.name }))]} />
        <ConfirmButton icon="save" key={JSON.stringify([selected, selection])} label="保存" confirmLabel={selected ? `启动时应用「${selected}」？` : "取消启动档案？"}
          disabled={!ready || unchanged || Boolean(selected && (!preview || modeError || gpuPowerError || graphicsModeError || displayError))} busy={busy} onConfirm={apply} />
      </div>
    </Surface>
  );
}
