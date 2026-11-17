import { Surface } from "../Surface";
import { Radio } from "../Radio";
import { Button } from "../Button";
import { useId } from "react";
import { OEM_POWER_MODES } from "../../api/client";
import { Icon } from "../Icon";
import { DiffPreview } from "./DiffPreview";
import { ModeArtwork } from "./ModeArtwork";
import { OemModeEffects } from "./OemModeEffects";
import { OemModeRestore } from "./OemModeRestore";
import { useOemPowerMode } from "./useOemPowerMode";
import { ParameterHelp } from "./ParameterHelp";
import { MODE_HELP } from "./parameterDescriptions";
import { gpuModeError } from "./gpu-values";

const PRESETS = [
  { mode: 1 }, { mode: 0 }, { mode: 3 }, { mode: 2 }, { mode: 4 },
];
const modeLabel = (mode: number) => OEM_POWER_MODES[mode] ?? `模式 ${mode}`;

export function OemPowerMode({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = useOemPowerMode(active, revision, onSettled);
  const { profile, draft, target, busy, reading, error } = model;
  const dirty = model.hasSelection && profile != null && draft != null;
  const gpuError = target ? target.unavailableReason ?? gpuModeError(target.gpu) : null;
  const group = useId();
  const disabled = busy || reading;
  const blocked = !!profile?.blockedReason;
  const presets = PRESETS.filter(({ mode }) => mode !== 4 || profile?.availableModes.includes(mode));
  const changes = profile && draft !== null ? [
    ...(draft !== profile.currentMode ? [{ label: "模式", from: modeLabel(profile.currentMode), to: modeLabel(draft) }] : []),
    ...(target?.acMode != null && target.acMode !== profile.acMode ? [{ label: "插电模式偏好", from: modeLabel(profile.acMode), to: modeLabel(target.acMode) }] : []),
  ] : [];
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="预设模式" aria-busy={disabled}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="performance" />预设模式</h2>
      <span className="ccc-chip" role="status">{reading ? "读取中…" : profile ? `当前使用 · ${modeLabel(profile.currentMode)}` : "当前模式未知"}</span>
      {profile && <span className="ccc-chip" aria-label="当前插电偏好">插电偏好 · {modeLabel(profile.acMode)}</span>}
    </div>
    <div className="ccc-mode-presets" role="radiogroup" aria-label="预设模式选择">
      {presets.map(({ mode }) => {
        const supported = profile?.availableModes.includes(mode) ?? false;
        const current = profile?.currentMode === mode;
        const status = reading ? "读取中…" : !profile ? "暂不可用" : !supported ? "固件不支持" : current ? "当前使用" : draft === mode ? "待应用" : null;
        return <Surface as="label" variant="card" className="ccc-mode-option" data-current={(current && supported && !reading) || undefined} key={mode}>
          <Radio appearance="overlay" name={group} checked={draft === mode} disabled={disabled || blocked || !supported}
            onChange={() => model.select(mode)} onClick={() => model.select(mode)} />
          <span className="ccc-mode-artwork">
            <ModeArtwork mode={mode} />
            <span className="ccc-mode-select-mark" aria-hidden="true"><Icon name="check-circle" size={24} /></span>
          </span>
          <span className="ccc-mode-copy">
            <span className="ccc-mode-option-head"><strong>{modeLabel(mode)}</strong>
              {status && <span className="ccc-mode-option-status">{status}</span>}
            </span>
          </span>
        </Surface>;
      })}
    </div>
    {profile && profile.availableModes.length === 0 && <span className="ccc-chip">固件未声明可用模式</span>}
    {dirty && changes.length > 0 && <DiffPreview items={changes} />}
    {dirty && changes.length === 0 && <span className="ccc-chip">重新应用 · {modeLabel(draft!)}</span>}
    {dirty && target && <OemModeEffects target={target} />}
    {dirty && target && (target.unavailableReason || !target.gpu) && <p className="ccc-control-error" role="alert">{gpuError}</p>}
    {dirty && !target && <p className="ccc-mode-hint">未读取到该模式的联动目标，请重试后再应用。</p>}
    {dirty && profile.softwareActive && <span className="ccc-chip">退出软件曲线，切换到原厂风扇策略</span>}
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {profile?.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    <div className="ccc-actions">
      <Button icon={busy ? "loading" : "apply"} type="button" aria-busy={busy}
        disabled={busy || reading || blocked || !dirty || !target?.revision || !!gpuError}
        onClick={() => { void model.apply(); }}>{busy ? "正在应用…" : "确认应用模式"}</Button>
      {(error || (dirty && !target)) && <Button icon="refresh" variant="ghost" size="compact" type="button" disabled={disabled} onClick={model.retry}>重试</Button>}
    </div>
    {profile?.restore && <OemModeRestore target={profile.restore} currentMode={profile.currentMode} currentAcMode={profile.acMode} currentEnergy={profile.energy}
      busy={busy} disabled={reading || blocked || !profile.canRestore} blockedReason={profile.restoreBlockedReason} label="恢复初始" onRestore={model.restore} />}
    <ParameterHelp title="模式说明" items={MODE_HELP} />
  </Surface>;
}
