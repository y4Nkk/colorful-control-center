import { useState } from "react";
import { CUSTOM_FAN_MODE, FAN_MODES, FAN_MODE_NOISELESS, NOISELESS_FAN_LABEL, PERFORMANCE_OPTION_LABELS,
  fanModeLabel, type FanProfile } from "../../api/client";
import { Button } from "../Button";
import { Dialog } from "../Dialog";
import { SegmentedControl } from "../SegmentedControl";
import { CoolingOptions } from "./CoolingOptions";
import { choiceDisabled, optionChoice, type CoolingChoice } from "./cooling-choice";
import { performanceState } from "./format";
import { ScaleSlider } from "./ScaleSlider";
import type { CoolingControl } from "./useCoolingControl";
import { useFanEdit } from "./useFanEdit";

type Pending = { source: string } & ({ choice: CoolingChoice } | { mode: number });

export function CoolingStrategy({ model, profile, active, revision }: {
  model: CoolingControl; profile: FanProfile; active: boolean; revision: number;
}) {
  const [pending, setPending] = useState<Pending | null>(null);
  const options = model.options.value;
  const custom = profile.currentMode === CUSTOM_FAN_MODE;
  const software = !!profile.software;
  const modes = FAN_MODES.filter(mode => profile.availableModes.includes(mode.id));
  const control = !model.readValid ? "状态未读取" : software ? "软件接管"
    : modes.some(mode => mode.id === profile.currentMode) ? null : fanModeLabel(profile.currentMode);
  const disabled = !model.readValid || !!profile.blockedReason;
  const optionsDisabled = disabled || !!model.options.error;
  const quietChoice = options && optionChoice(options, "quietMode", true, custom);
  const quietLinked = !!quietChoice && (options?.turboActive || custom && !!options?.quietFan) && !software;
  const quietDisabled = options === null || !!model.options.error ||
    !!(quietLinked && quietChoice && choiceDisabled(options, quietChoice, custom, software));
  const source = JSON.stringify([revision, profile, options?.revision, options?.blockedReason, optionsDisabled]);
  if (pending && (!active || pending.source !== source)) setPending(null);
  const choose = (choice: CoolingChoice) => {
    if (!options || optionsDisabled || model.busy || choiceDisabled(options, choice, custom, software)) return;
    if (choice.confirm) setPending({ source, choice });
    else void model.apply(options.revision, choice.change);
  };
  const selectMode = (mode: number) => {
    if (mode === FAN_MODE_NOISELESS && quietLinked && quietChoice) choose(quietChoice);
    else if (software) setPending({ source, mode });
    else void model.setMode(mode);
  };
  const confirm = () => {
    if (!pending || pending.source !== source || model.busy) return;
    setPending(null);
    if ("mode" in pending) void model.setMode(pending.mode);
    else if (options) void model.apply(options.revision, pending.choice.change);
  };
  return <section className="ccc-cooling-strategy" aria-label="整机策略">
    <div className="ccc-cooling-strategy-head">
      <h3>整机策略</h3>
      {control && <span className="ccc-cooling-state" role="status" aria-label="当前控制">{control}</span>}
    </div>
    <div className="ccc-cooling-controls">
      <div className="ccc-cooling-field">
        <h4>风扇模式</h4>
        <SegmentedControl label="风扇模式" balancedWrap value={software ? "" : String(profile.currentMode)} disabled={disabled}
          busy={model.busy} onChange={value => selectMode(Number(value))}
          options={modes.map(mode => ({
            value: String(mode.id), label: mode.label,
            disabled: mode.id === CUSTOM_FAN_MODE && (!profile.customSupported || !!profile.customBlockedReason) ||
              mode.id === FAN_MODE_NOISELESS && quietDisabled,
          }))} />
        {software && !disabled && <p className="ccc-control-caption">选择「自动」可停止当前的软件风扇曲线。</p>}
        {profile.customBlockedReason && <p className="ccc-control-caption">{profile.customBlockedReason}</p>}
        {options?.currentMode === 2 && options.quietFan && !options.quietFan.remembered &&
          <p className="ccc-control-caption">切换预设后，此设备会重置性能档的寂静设置。</p>}
      </div>
      <FanOffset model={model} profile={profile} disabled={disabled} />
    </div>
    {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    {options && <CoolingOptions options={options} busy={model.busy} disabled={optionsDisabled}
      custom={custom} software={software} onChoice={choose} />}
    {pending && active && pending.source === source && !disabled && <CoolingConfirmation pending={pending} custom={custom}
      busy={model.busy} onCancel={() => setPending(null)} onConfirm={confirm} />}
  </section>;
}

function FanOffset({ model, profile, disabled }: { model: CoolingControl; profile: FanProfile; disabled: boolean }) {
  const source = JSON.stringify([profile.currentMode, profile.offsetSlot, profile.offsetPct]);
  const offset = useFanEdit(source, profile.offsetPct ?? 0, model.setOffset);
  return <div className="ccc-cooling-field">
    <ScaleSlider label="加速偏移" unit="%" min={0} max={100} magnetInterval={10} current={profile.offsetPct ?? 0} draft={offset.value}
      unavailable={profile.offsetPct === null} disabled={disabled || profile.offsetBlockedReason !== null} busy={model.busy}
      onChange={value => offset.update(() => value)} onCommit={value => offset.commit(() => value)} onCancel={offset.cancel} />
    {profile.offsetBlockedReason && profile.offsetBlockedReason !== profile.blockedReason &&
      <p className="ccc-control-caption">{profile.offsetBlockedReason}</p>}
  </div>;
}

function CoolingConfirmation({ pending, custom, busy, onCancel, onConfirm }: {
  pending: Pending; custom: boolean; busy: boolean; onCancel: () => void; onConfirm: () => void;
}) {
  const choice = "choice" in pending ? pending.choice : null;
  const change = choice?.change;
  const target = choice?.fanTarget === "quiet" ? NOISELESS_FAN_LABEL : choice?.fanTarget === "auto" ? "自动" : null;
  return <Dialog title="确认散热策略" width="narrow" open onClose={onCancel} actions={<>
    <Button variant="secondary" onClick={onCancel}>取消</Button>
    <Button disabled={busy} onClick={onConfirm}>确认应用</Button>
  </>}>
    {"mode" in pending && <p>切换到「{fanModeLabel(pending.mode)}」会停止当前的软件风扇曲线。</p>}
    {target && <p>{custom ? "从自定义固件风扇曲线切换到" : "风扇切换到"}「{target}」。</p>}
    {change && (["turboFan", "dtt"] as const).map(key => change.values[key] !== null &&
      <p key={key}>{PERFORMANCE_OPTION_LABELS[key]} · {performanceState(change.values[key])}</p>)}
    {change?.quietFan === "entertainmentOn" && <p>记住娱乐档的寂静偏好。</p>}
    {change?.quietFan === "entertainmentOff" && <p>关闭娱乐档的寂静偏好。</p>}
  </Dialog>;
}
