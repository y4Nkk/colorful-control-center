import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { Tabs } from "../Tabs";
import { useId, useState, type ReactNode } from "react";
import { CUSTOM_FAN_MODE, FAN_LABELS, FAN_MODES, fanModeLabel, type FanId, type FanProfile, type HardwareSnapshot } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { FanCurveEditor } from "./FanCurveEditor";
import { FanCurveSection } from "./FanCurveSection";
import { FanLiveReadings, fanTemperature } from "./FanLiveReadings";
import { ScaleSlider } from "./ScaleSlider";
import { normalizeDraft } from "./curve";
import { useFanControl } from "./useFanControl";
import { RpmCurveEditor } from "./RpmCurveEditor";
import { ParameterDescriptions } from "./ParameterHelp";
import { FAN_HELP } from "./parameterDescriptions";
import { useFanEdit } from "./useFanEdit";

export function FanPanel({ active, revision, onSettled, live, stale }: {
  active: boolean; revision: number; onSettled: () => void; live: HardwareSnapshot | null; stale: boolean;
}) {
  const model = useFanControl(active, revision, onSettled);
  const [fanId, setFanId] = useState<FanId>("cpu");
  const { profile } = model;
  const recovery = profile && <FanRecovery profile={profile} model={model} />;
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="风扇设置" aria-busy={model.busy || undefined}>
      <div className="ccc-fan-panel-head"><h2 className="ccc-panel-title"><Icon name="fan" />风扇设置</h2>
        <span className="ccc-control-caption" role="status">{model.busy ? "应用中…" : null}</span>
        <div className="ccc-fan-panel-help"><DetailsDialog title="风扇说明与诊断" label="说明与诊断" icon={null}>
          <section><h3>参数说明</h3><ParameterDescriptions items={FAN_HELP} /></section>
          {profile && <section><h3>安全诊断</h3><p className="ccc-control-caption">{profile.safetyStatus}</p>
            {profile.warnings.map(warning => <p key={warning} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
          </section>}
        </DetailsDialog></div>
      </div>
      {model.error && <div role="alert" className="ccc-control-error">{model.error}
        <Button icon="refresh" variant="ghost" size="compact" type="button" disabled={model.busy} onClick={model.retry}>重试</Button></div>}
      {profile ? <FanSettings model={model} profile={profile} live={live} stale={stale} fanId={fanId} onFanChange={setFanId} recovery={recovery} />
        : !model.error && <span className="ccc-control-caption" role="status">读取中…</span>}
    </Surface>
  );
}

type FanModel = ReturnType<typeof useFanControl>;

function FanSettings({ model, profile, live, stale, fanId, onFanChange, recovery }: {
  model: FanModel; profile: FanProfile; live: HardwareSnapshot | null; stale: boolean;
  fanId: FanId; onFanChange: (id: FanId) => void; recovery?: ReactNode;
}) {
  const fans = [...profile.fans.filter(fan => fan.present), ...profile.rpmFans];
  const selected = fans.find(item => item.id === fanId) ?? fans[0];
  const editable = profile.customSupported && model.readValid && !profile.blockedReason &&
    !profile.customBlockedReason && !profile.software;
  const curveStatus = profile.software || profile.currentMode !== CUSTOM_FAN_MODE ? "自定义曲线未启用" : undefined;
  const control = !model.readValid ? "状态未读取" : profile.software ? "软件接管" : `固件 · ${fanModeLabel(profile.currentMode)}`;
  return <div className="ccc-fan-layout">
    {selected ? <Tabs label="选择风扇" value={selected.id} onChange={onFanChange} tabs={fans.map(fan => {
      const source = JSON.stringify([profile.currentMode, fan]);
      return { value: fan.id, label: FAN_LABELS[fan.id], content: <div className="ccc-fan-details">
        <FanLiveReadings live={live} stale={stale} id={fan.id} control={control} />
        {"points" in fan ? <RpmCurveEditor status={curveStatus} fanName={FAN_LABELS[fan.id]} source={source}
          current={fan.points} maximumRpm={fan.maximumRpm} editable={editable} busy={model.busy} onCommit={next => model.changeRpm(fan.id, next)} />
          : <FanCurveEditor status={curveStatus} fanName={FAN_LABELS[fan.id]} anchors={fan} source={source}
            current={normalizeDraft(fan, fan)} editable={editable} busy={model.busy}
            liveTempC={fanTemperature(live, fan.id)} stale={stale} onCommit={next => model.changeCurve(fan.id, next)} />}
      </div> };
    })} /> : <>
      <FanLiveReadings live={live} stale={stale} control={control} />
      <FanCurveSection status="未读取到风扇曲线" />
    </>}
    <section className="ccc-fan-settings">
      <h3>整机设置</h3>
      <FirmwareSettings model={model} profile={profile} />
      {recovery}
    </section>
  </div>;
}

function FirmwareSettings({ model, profile }: { model: FanModel; profile: FanProfile }) {
  const disabled = !model.readValid || !!profile.blockedReason;
  const source = JSON.stringify([profile.currentMode, profile.offsetSlot, profile.offsetPct]);
  const offset = useFanEdit(source, profile.offsetPct ?? 0, model.setOffset);
  return <>
    <div className="ccc-fan-setting-controls">
      <div className="ccc-fan-setting">
        <span className="ccc-control-caption">风扇模式</span>
        <SegmentedControl label="风扇模式" value={profile.software ? "" : String(profile.currentMode)}
          disabled={disabled} busy={model.busy} onChange={value => model.setMode(Number(value))}
          options={FAN_MODES.filter(mode => profile.availableModes.includes(mode.id)).map(mode => ({
            value: String(mode.id), label: mode.label,
            disabled: mode.id === CUSTOM_FAN_MODE && (!profile.customSupported || !!profile.customBlockedReason),
          }))} />
        {profile.software && !disabled && <p className="ccc-control-caption">选择固件模式可交还风扇控制。</p>}
        {profile.customBlockedReason && <p className="ccc-control-caption">{profile.customBlockedReason}</p>}
      </div>
      <div className="ccc-fan-setting">
        <ScaleSlider label="加速偏移" unit="%" min={0} max={100} magnetInterval={10} current={profile.offsetPct ?? 0} draft={offset.value}
          unavailable={profile.offsetPct === null}
          disabled={disabled || profile.offsetBlockedReason !== null} busy={model.busy} onChange={value => offset.update(() => value)}
          onCommit={value => offset.commit(() => value)} onCancel={offset.cancel} />
        {profile.offsetBlockedReason && profile.offsetBlockedReason !== profile.blockedReason &&
          <p className="ccc-control-caption">{profile.offsetBlockedReason}</p>}
      </div>
    </div>
    {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
  </>;
}

function FanRecovery({ profile, model }: { profile: FanProfile; model: FanModel }) {
  const [expanded, setExpanded] = useState(false);
  const id = useId();
  const revision = JSON.stringify(profile);
  const blocked = model.busy || !model.readValid || !!profile.blockedReason;
  return <div>
    <Button variant="ghost" size="compact" icon={expanded ? "chevron-down" : "chevron-right"}
      aria-expanded={expanded} aria-controls={id} disabled={model.busy} onClick={() => setExpanded(value => !value)}>恢复设置</Button>
    <div className="ccc-fan-recovery-actions" id={id} hidden={!expanded}>
      <ConfigurationAction label="风扇设置" revision={revision} restore disabled={!profile.canRestore} blocked={blocked} onConfirm={model.restore}>
        <p>恢复首次调整前记录的风扇设置。</p>
      </ConfigurationAction>
      <ConfigurationAction label="固件默认风扇设置" revision={revision} restore
        blocked={blocked || !profile.customSupported || (profile.curveKind === "rpmTable" && !!profile.customBlockedReason)}
        onConfirm={model.restoreDefault}>
        <p>{profile.offsetPct === null ? "使用固件默认值重置风扇曲线。" : "使用固件默认值重置风扇曲线，并清零加速偏移。"}</p>
      </ConfigurationAction>
    </div>
  </div>;
}
