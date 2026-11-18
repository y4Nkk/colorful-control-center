import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { useEffect, useState } from "react";
import { FAN_LABELS, FAN_MODES, type FanId } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { DiffPreview } from "./DiffPreview";
import { FanCurveEditor } from "./FanCurveEditor";
import { ScaleSlider } from "./ScaleSlider";
import { currentOf } from "./fanDrafts";
import { useFanControl } from "./useFanControl";
import { RpmCurveEditor } from "./RpmCurveEditor";
import { SoftwareFanPanel } from "./SoftwareFanPanel";
import { ParameterHelp } from "./ParameterHelp";
import { FAN_HELP } from "./parameterDescriptions";

export function FanPanel({ active, revision, onSettled, temps }: { active: boolean; revision: number; onSettled: () => void; temps: { cpu: number | null; gpu: number | null } }) {
  const [softwareBusy, setSoftwareBusy] = useState(false);
  const model = useFanControl(active, softwareBusy, revision, onSettled);
  const [tab, setTab] = useState<FanId>("cpu");
  const [executor, setExecutor] = useState<"firmware" | "software">("firmware");
  const { profile } = model;
  const softwareRunning = profile?.software != null;
  useEffect(() => { if (softwareRunning) setExecutor("software"); }, [softwareRunning]);
  const fans = [...(profile?.fans.filter((fan) => fan.present) ?? []), ...(profile?.rpmFans ?? [])];
  const selected = fans.find((item) => item.id === tab) ?? fans[0];
  const fan = profile?.fans.find((item) => item.id === selected?.id);
  const rpmFan = profile?.rpmFans.find((item) => item.id === selected?.id);
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="风扇曲线">
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="fan" />风扇曲线</h2>
        <span className="ccc-chip">{profile ? softwareRunning ? "软件接管" : (FAN_MODES.find(mode => mode.id === profile.currentMode)?.label ?? "模式未知") : "未读取"}</span>
        <div className="ccc-module-tools"><ParameterHelp title="风扇曲线参数说明" items={FAN_HELP} />
          {profile && <DetailsDialog title="风扇诊断" label="诊断">
            <p>{profile.safetyStatus}</p>
            {profile.warnings.map(warning => <p key={warning}>{warning}</p>)}
          </DetailsDialog>}</div>
      </div>
      {model.error && <div role="alert" className="ccc-control-error">{model.error}
        <Button icon="refresh" variant="ghost" size="compact" type="button" disabled={model.busy} onClick={model.retry}>重试</Button></div>}
      {profile ? <>
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
        <SegmentedControl className="ccc-fan-executor" label="风扇曲线执行方式" value={executor}
          disabled={model.busy || softwareBusy} onChange={value => setExecutor(value as typeof executor)}
          options={[{ value: "firmware", label: "固件曲线" }, { value: "software", label: "软件曲线",
            disabled: !profile.softwareSupported, title: !profile.softwareSupported ? "当前固件尚未核对软件控制协议" : undefined }]} />
        <div className="ccc-fan-content" hidden={executor !== "firmware"}>
        <div className="ccc-fan-toolbar">
          {selected && <SegmentedControl label="选择风扇" value={selected.id} onChange={value => setTab(value as FanId)}
            options={fans.map(item => ({ value: item.id, label: FAN_LABELS[item.id] }))} />}
          <SegmentedControl label="风扇模式" value={String(model.mode)} disabled={model.busy || !!profile.blockedReason}
            onChange={value => model.setMode(Number(value))}
            options={FAN_MODES.filter(mode => profile.availableModes.includes(mode.id)).map(mode => ({
              value: String(mode.id), label: mode.label, disabled: mode.id === 6 && !profile.customSupported }))} />
        </div>
        {fan && <FanCurveEditor title={`${FAN_LABELS[fan.id]} 温度与占空比曲线`} anchors={{ t1: fan.t1, d1Pct: fan.d1Pct }}
          current={currentOf(fan)} draft={model.drafts[fan.id] ?? currentOf(fan)}
          editable={profile.customSupported && !model.busy} liveTempC={fan.id === "cpu" ? temps.cpu : temps.gpu}
          onChange={(next) => model.changeCurve(fan.id, next)} />}
        {rpmFan && <RpmCurveEditor key={rpmFan.id} current={rpmFan.points}
          draft={model.rpmDrafts[rpmFan.id] ?? rpmFan.points} maximumRpm={rpmFan.maximumRpm}
          editable={profile.customSupported && !model.busy} onChange={(next) => model.changeRpm(rpmFan.id, next)} />}
        <ScaleSlider label="加速偏移" unit="%" min={0} max={100} current={profile.offsetPct} draft={model.offset} disabled={model.busy} onChange={model.setOffset} />
        {executor === "firmware" && <FanChanges model={model} />}
        </div>
        {profile.softwareSupported && <div hidden={executor !== "software"}><SoftwareFanPanel visible={executor === "software"}
          readValid={model.readValid} profile={profile} refresh={model.refresh} onBusy={(value) => { setSoftwareBusy(value); if (!value) onSettled(); }} /></div>}
        <ConfigurationAction label="风扇设置" revision={JSON.stringify(profile)} restore
          disabled={model.busy || softwareBusy || !model.readValid || !profile.canRestore || !!profile.blockedReason} onConfirm={model.restore}>
          <p>恢复首次调整前的风扇模式、曲线和加速偏移。</p>
        </ConfigurationAction>
        <ConfigurationAction label="固件默认风扇设置" revision={JSON.stringify(profile)} restore
          disabled={model.busy || softwareBusy || !model.readValid || !!profile.blockedReason} onConfirm={model.restoreDefault}>
          <p>使用固件默认值重置风扇曲线及偏移。</p>
        </ConfigurationAction>
      </> : !model.error && <span className="ccc-chip">读取中…</span>}
    </Surface>
  );
}

function FanChanges({ model }: { model: ReturnType<typeof useFanControl> }) {
  const { profile, drafts, rpmDrafts, mode, offset, busy } = model;
  if (!profile) return null;
  const items = profile.fans.filter((fan) => fan.present).flatMap((fan) => {
    const current = currentOf(fan); const draft = drafts[fan.id] ?? current;
    return (Object.keys(current) as Array<keyof typeof current>).filter((key) => current[key] !== draft[key])
      .map((key) => ({ label: `${FAN_LABELS[fan.id]} ${key.startsWith("t") ? key.toUpperCase() : key.slice(0, 2).toUpperCase()}`,
        from: `${current[key]}${key.startsWith("t") ? "°" : "%"}`, to: `${draft[key]}${key.startsWith("t") ? "°" : "%"}` }));
  });
  for (const fan of profile.rpmFans) {
    (rpmDrafts[fan.id] ?? fan.points).forEach((point, i) => {
      const before = fan.points[i];
      if (point.tempC !== before.tempC || point.rpmPct !== before.rpmPct) items.push({
        label: `${FAN_LABELS[fan.id]} 第 ${i + 1} 级`, from: `${before.tempC}° / ${before.rpmPct}%`, to: `${point.tempC}° / ${point.rpmPct}%`,
      });
    });
  }
  const label = (id: number) => FAN_MODES.find((item) => item.id === id)?.label ?? String(id);
  if (profile.software) items.push({ label: "执行方式", from: "软件曲线", to: "固件曲线" });
  if (mode !== profile.currentMode) items.push({ label: "模式", from: label(profile.currentMode), to: label(mode) });
  if (offset !== profile.offsetPct) items.push({ label: "加速", from: `${profile.offsetPct}%`, to: `${offset}%` });
  return <ConfigurationAction label="固件曲线" revision={JSON.stringify([profile, items])}
    disabled={busy || !model.readValid || !model.dirty || !!profile.blockedReason} onConfirm={model.apply}>
    <DiffPreview items={items} />
  </ConfigurationAction>;
}
