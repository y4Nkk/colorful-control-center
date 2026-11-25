import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { Tabs } from "../Tabs";
import { useState } from "react";
import { CUSTOM_FAN_MODE, FAN_LABELS, type FanId, type FanProfile } from "../../api/client";
import { RestoreSettings } from "../RestoreSettings";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { CoolingStrategy } from "./CoolingStrategy";
import { FanCurveEditor } from "./FanCurveEditor";
import { FanCurveSection } from "./FanCurveSection";
import { FanLiveReadings } from "./FanLiveReadings";
import { normalizeDraft } from "./curve";
import { useCoolingControl, type CoolingControl } from "./useCoolingControl";
import { RpmCurveEditor } from "./RpmCurveEditor";
import { ParameterDescriptions } from "./ParameterHelp";
import { FAN_HELP } from "./parameterDescriptions";

export function CoolingPanel({ active, revision, onSettled }: {
  active: boolean; revision: number; onSettled: (source: "fan" | "options") => void;
}) {
  const model = useCoolingControl(active, revision, onSettled);
  const [fanId, setFanId] = useState<FanId>("cpu");
  const { profile } = model;
  const errors = [model.actionError, model.fanError, model.options.error].filter(Boolean);
  const warnings = [...new Set([...(profile?.warnings ?? []), ...(model.options.value?.warnings ?? [])])];
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="散热控制" aria-busy={model.busy || undefined}>
      <div className="ccc-panel-head ccc-cooling-head">
        <h2 className="ccc-panel-title"><Icon name="fan" />散热控制</h2>
        <div className="ccc-module-tools">
          <span className="ccc-control-caption" role="status">{model.writing ? "应用中…" : null}</span>
          <DetailsDialog title="风扇说明与诊断" label="说明与诊断" icon={null}>
            <section><h3>参数说明</h3><ParameterDescriptions items={FAN_HELP} /></section>
            {(profile || warnings.length > 0) && <section><h3>安全诊断</h3>{profile && <p className="ccc-control-caption">{profile.safetyStatus}</p>}
              {warnings.map(warning => <p key={warning} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
            </section>}
          </DetailsDialog>
          <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取散热控制" disabled={model.busy} onClick={model.retry} />
        </div>
      </div>
      {!!errors.length && <div role="alert" className="ccc-control-error">{[...new Set(errors)].join("；")}
        <Button icon="refresh" variant="ghost" size="compact" type="button" disabled={model.busy} onClick={model.retry}>重试</Button></div>}
      <OemConflictAction issues={[model.actionError, model.options.value?.blockedReason]} disabled={model.busy} onClosed={model.retry} />
      {((model.fanError && profile) || (model.options.error && model.options.value)) &&
        <p className="ccc-control-caption">显示上次读取的设置。重新读取后可修改。</p>}
      {profile ? <div className="ccc-cooling-layout">
        <div className="ccc-cooling-policy">
          <CoolingStrategy model={model} profile={profile} active={active} revision={revision} />
          <FanRecovery profile={profile} model={model} />
        </div>
        <FanWorkspace model={model} profile={profile} active={active} fanId={fanId} onFanChange={setFanId} />
      </div> : !errors.length && <span className="ccc-control-caption" role="status">读取中…</span>}
    </Surface>
  );
}

function FanWorkspace({ model, profile, active, fanId, onFanChange }: {
  model: CoolingControl; profile: FanProfile; active: boolean;
  fanId: FanId; onFanChange: (id: FanId) => void;
}) {
  const fans = [...profile.fans.filter(fan => fan.present), ...profile.rpmFans];
  const selected = fans.find(item => item.id === fanId) ?? fans[0];
  const editable = profile.customSupported && model.readValid && !profile.blockedReason &&
    !profile.customBlockedReason && !profile.software;
  const curveInactive = !!profile.software || profile.currentMode !== CUSTOM_FAN_MODE;
  const curveStatus = curveInactive ? editable ? "调整曲线后将启用自定义模式" : "自定义曲线未启用" : undefined;
  return <section className="ccc-fan-workspace" aria-label="风扇实况与曲线">
    <h3>风扇实况与曲线</h3>
    {selected ? <Tabs label="选择风扇" divider={false} value={selected.id} onChange={onFanChange} tabs={fans.map(fan => {
      const source = JSON.stringify([profile.currentMode, fan]);
      return { value: fan.id, label: FAN_LABELS[fan.id], content: <div className="ccc-fan-details">
        <FanLiveReadings active={active} id={fan.id} />
        {"points" in fan ? <RpmCurveEditor status={curveStatus} fanName={FAN_LABELS[fan.id]} source={source}
          current={fan.points} maximumRpm={fan.maximumRpm} editable={editable} busy={model.busy} onCommit={next => model.changeRpm(fan.id, next)} />
          : <FanCurveEditor status={curveStatus} fanName={FAN_LABELS[fan.id]} anchors={fan} source={source}
            current={normalizeDraft(fan, fan)} editable={editable} busy={model.busy} onCommit={next => model.changeCurve(fan.id, next)} />}
      </div> };
    })} /> : <FanCurveSection status="未读取到风扇曲线" />}
  </section>;
}

function FanRecovery({ profile, model }: { profile: FanProfile; model: CoolingControl }) {
  const revision = profile.restoreRevision;
  const blocked = model.busy || !model.readValid || !!profile.blockedReason;
  const canRestoreDefault = profile.customSupported && (profile.curveKind !== "rpmTable" || !profile.customBlockedReason);
  return <RestoreSettings label="散热控制" triggerLabel="恢复设置" blocked={blocked}
    note={<p>同时勾选两项时，先恢复初始设置，再应用固件默认值。</p>}
    options={[
      { label: "初始风扇设置", revision: profile.canRestore ? revision : null, target: "initial" as const,
        preview: <p>{profile.canRestore ? "恢复首次调整前记录的风扇设置。" : "尚无首次调整记录。"}</p> },
      { label: "固件默认风扇设置", revision: canRestoreDefault ? revision : null, target: "firmwareDefault" as const,
        preview: <p>{!canRestoreDefault ? "当前固件默认设置不可恢复。"
          : `恢复固件默认曲线并切换至自动模式${profile.offsetPct === null ? "。" : "，同时清零加速偏移。"}`}</p> },
    ]} onRestore={selected => model.restore(selected.length === 2 ? "initialAndFirmwareDefault" : selected[0].target, revision)} />;
}
