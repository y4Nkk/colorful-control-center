import { Surface } from "../Surface";
import { Radio } from "../Radio";
import { Button } from "../Button";
import { Dialog } from "../Dialog";
import { DetailsDialog } from "../DetailsDialog";
import { useId } from "react";
import { OEM_POWER_MODES } from "../../api/client";
import { Icon } from "../Icon";
import { ModeArtwork } from "./ModeArtwork";
import { OemModeRestore } from "./OemModeRestore";
import { useOemPowerMode } from "./useOemPowerMode";
import { ParameterDescriptions } from "./ParameterHelp";
import { MODE_HELP } from "./parameterDescriptions";

const PRESETS = [
  { mode: 1 }, { mode: 0 }, { mode: 3 }, { mode: 2 }, { mode: 4 },
];
const modeLabel = (mode: number) => OEM_POWER_MODES[mode] ?? `模式 ${mode}`;

export function OemPowerMode({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = useOemPowerMode(active, revision, onSettled);
  const { profile, pending, reading, error, fnKeyPrompt } = model;
  const group = useId();
  const busy = pending !== null;
  const disabled = busy || reading;
  const blocked = !!profile?.blockedReason;
  const presets = PRESETS.filter(({ mode }) => mode !== 4 || profile?.availableModes.includes(mode));
  const hasModes = (profile?.availableModes.length ?? 0) > 0;
  const status = typeof pending === "number" ? `正在切换到${modeLabel(pending)}…` : busy ? "正在恢复…"
    : reading ? "读取中…" : !profile ? "当前模式未知"
    : presets.some(({ mode }) => mode === profile.currentMode) ? null : `当前模式 · ${modeLabel(profile.currentMode)}`;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="预设模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="performance" />预设模式</h2>
      <span className="ccc-control-caption" role="status">{status}</span>
      <div className="ccc-module-tools"><DetailsDialog label="详情" title="模式说明">
        {profile && <p>插电偏好 · {modeLabel(profile.acMode)}</p>}
        {profile?.softwareActive && <p>切换预设会退出当前软件风扇曲线。</p>}
        <ParameterDescriptions items={MODE_HELP} />
      </DetailsDialog></div>
    </div>
    <div className="ccc-mode-presets" role="radiogroup" aria-label="预设模式选择" aria-busy={disabled}>
      {presets.map(({ mode }) => {
        const supported = profile?.availableModes.includes(mode) ?? false;
        const current = profile?.currentMode === mode;
        return <Surface as="label" variant="card" className="ccc-mode-option" key={mode}>
          <Radio appearance="overlay" name={group} checked={current} disabled={blocked || !supported} aria-disabled={disabled || undefined}
            onChange={() => { if (!disabled) void model.switchMode(mode); }}
            onClick={event => {
              if (disabled) event.preventDefault();
              else if (current) void model.switchMode(mode);
            }} />
          <ModeArtwork mode={mode} />
          <span className="ccc-mode-copy">
            <strong>{modeLabel(mode)}</strong>
            {hasModes && !supported && <span className="ccc-mode-option-status">固件不支持</span>}
          </span>
          <span className="ccc-mode-select-mark" aria-hidden="true"><Icon name="check" size={16} /></span>
        </Surface>;
      })}
    </div>
    {profile && !hasModes && <p className="ccc-control-caption">固件未声明可用模式</p>}
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {profile?.blockedReason && profile.blockedReason !== error && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    {error && <Button className="ccc-inline-tool" icon="refresh" variant="ghost" size="compact" disabled={disabled} onClick={model.retry}>重新读取</Button>}
    {profile?.restore && <OemModeRestore target={profile.restore} currentMode={profile.currentMode} currentAcMode={profile.acMode} currentEnergy={profile.energy}
      busy={busy} disabled={reading || blocked || !profile.canRestore} blockedReason={profile.restoreBlockedReason ?? profile.blockedReason} onRestore={model.restore} />}
    <Dialog title="关闭原厂 FnKey" width="narrow" open={active && fnKeyPrompt !== null} busy={busy} onClose={model.cancelFnKey}
      actions={<><Button variant="secondary" disabled={busy} onClick={model.cancelFnKey}>取消</Button>
        <Button icon={busy ? "loading" : undefined} disabled={disabled || blocked} onClick={() => void model.confirmFnKey()}>
          {busy ? "正在关闭并切换…" : "关闭并切换"}
        </Button></>}>
      {fnKeyPrompt && <p>关闭原厂 FnKey 后，控制台会切换到「{modeLabel(fnKeyPrompt.target.mode)}」。</p>}
      <p>部分原厂 Fn 热键和屏显会暂停。可以从开始菜单打开「Fn hotkeys and OSD」恢复。</p>
    </Dialog>
  </Surface>;
}
