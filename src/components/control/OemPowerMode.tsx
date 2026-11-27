import { Surface } from "../Surface";
import { SegmentedControl } from "../SegmentedControl";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { OEM_POWER_MODES } from "../../api/client";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { PresetArtwork, PresetBackdrop } from "./PresetArtwork";
import { useOemPowerMode } from "./useOemPowerMode";
import { ParameterDescriptions } from "./ParameterHelp";
import { MODE_HELP } from "./parameterDescriptions";

const modeLabel = (mode: number) => OEM_POWER_MODES[mode] ?? "未知";

export function OemPowerMode({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = useOemPowerMode(active, revision, onSettled);
  const { profile, pending, reading, error, readValid } = model;
  const currentMode = readValid ? profile?.currentMode : undefined;
  const currentName = currentMode === undefined ? undefined : OEM_POWER_MODES[currentMode];
  const targetMode = typeof pending === "number" ? pending : undefined;
  const busy = pending !== null;
  const disabled = busy || reading;
  const blocked = !readValid || !!profile?.blockedReason;
  const hasModes = (profile?.availableModes.length ?? 0) > 0;
  const presets = [1, 0, 3, 2];
  const unsupported = hasModes ? presets.filter(mode => !profile?.availableModes.includes(mode)) : [];
  const options = presets.map(mode => ({
    value: String(mode),
    label: modeLabel(mode),
    disabled: !profile?.availableModes.includes(mode),
    tooltip: unsupported.includes(mode) ? "固件不支持" : undefined,
  }));
  const status = targetMode !== undefined ? `正在切换到${modeLabel(targetMode)}…` : null;
  const selectMode = (value: string) => {
    const mode = Number(value);
    void model.switchMode(mode, modeLabel(mode));
  };
  return <Surface as="section" variant="group" className="ccc-panel ccc-preset-panel" aria-label="预设模式">
    <PresetBackdrop mode={currentMode} />
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="performance" />预设模式</h2>
      <span className="ccc-control-caption" role="status">{status && <Loading embedded label={status} />}</span>
      <div className="ccc-module-tools"><DetailsDialog title="模式说明">
        {readValid && profile && <p className="ccc-control-caption">插电偏好 · {modeLabel(profile.acMode)}</p>}
        {readValid && profile?.softwareActive && <p className="ccc-control-caption is-warn"><Icon name="warning" size={16} />切换预设会退出当前软件风扇曲线。</p>}
        <ParameterDescriptions items={MODE_HELP} />
      </DetailsDialog></div>
    </div>
    <PresetArtwork mode={currentMode} active={active} />
    <p className="ccc-preset-current" aria-live="polite">
      {reading && !profile && !error ? <Loading embedded label="正在读取当前模式…" />
        : currentName ? `${currentName}模式` : "当前模式未知"}
    </p>
    <SegmentedControl className="ccc-preset-switcher" label="预设模式选择" value={currentMode === undefined ? "" : String(currentMode)}
      options={options} disabled={blocked} busy={disabled} pendingValue={targetMode === undefined ? undefined : String(targetMode)}
      onChange={selectMode} onReselect={selectMode} />
    {unsupported.length > 0 && <p className="ccc-control-caption">固件不支持 · {unsupported.map(modeLabel).join("、")}</p>}
    {profile && !hasModes && <p className="ccc-control-caption">固件未声明可用模式</p>}
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {profile?.blockedReason && profile.blockedReason !== error && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    <OemConflictAction issues={[error, profile?.blockedReason]} disabled={disabled} onClosed={model.retry} />
    {error && <Button className="ccc-inline-tool" icon="refresh" variant="ghost" size="compact" disabled={disabled} onClick={model.retry}>重新读取</Button>}
  </Surface>;
}
