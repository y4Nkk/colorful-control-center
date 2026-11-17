import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { useEffect, useState } from "react";
import { FAN_LABELS, FAN_MODES, type FanId } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
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
        <span className="ccc-chip">{profile?.safetyStatus || "读取中…"}</span></div>
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
          <SegmentedControl label="选择风扇" value={selected.id} onChange={value => setTab(value as FanId)}
            options={fans.map(item => ({ value: item.id, label: FAN_LABELS[item.id] }))} />
          <SegmentedControl label="风扇模式" value={String(model.mode)} disabled={model.busy}
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
        <FanActions model={model} />
        </div>
        {profile.softwareSupported && <div hidden={executor !== "software"}><SoftwareFanPanel profile={profile} refresh={model.refresh} onBusy={(value) => { setSoftwareBusy(value); if (!value) onSettled(); }} /></div>}
      </> : !model.error && <span className="ccc-chip">读取中…</span>}
      <ParameterHelp title="风扇曲线参数说明" items={FAN_HELP} />
    </Surface>
  );
}

function FanActions({ model }: { model: ReturnType<typeof useFanControl> }) {
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
  return <>
    <DiffPreview items={items} />
    <div className="ccc-actions">
      <ConfirmButton key={`${profile.blockedReason}-${JSON.stringify(items)}`} label="应用" confirmLabel={`确认应用 ${items.length} 项变更？`}
        disabled={!model.dirty || !!profile.blockedReason} busy={busy} onConfirm={model.apply} />
      <ConfirmButton icon="undo" key={`restore-${profile.blockedReason}`} variant="secondary" label="恢复初始" confirmLabel="恢复风扇初始设置？" disabled={!profile.canRestore || !!profile.blockedReason}
        busy={busy} onConfirm={model.restore} />
    </div>
    <Collapsible summary="诊断">
      {profile.warnings.map((warning) => <p key={warning}>{warning}</p>)}
      <ConfirmButton key={`default-${profile.blockedReason}`} label="固件默认" confirmLabel="恢复固件默认风扇设置？" disabled={!!profile.blockedReason} busy={busy} onConfirm={model.restoreDefault} />
    </Collapsible>
  </>;
}
