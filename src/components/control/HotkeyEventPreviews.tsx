import type { HotkeyProfile, HotkeyScope } from "../../api/client";
import { GpuTurboFacts } from "./GpuTurboFacts";
import { BatteryThresholdFacts } from "./BatteryThresholdFacts";
import { PdEventFacts } from "./PdEventFacts";
import { BatteryAlertFacts } from "./BatteryAlertFacts";
import { BatteryLowBrightnessFacts } from "./BatteryLowBrightnessFacts";
import { SystemActionFacts } from "./SystemActionFacts";
import { BatteryLowActionFacts } from "./BatteryLowActionFacts";
import { ModeCycleFacts } from "./ModeCycleFacts";

type Props = { profile: HotkeyProfile | null; selected: ReadonlySet<HotkeyScope>; draftActions: HotkeyScope[] };

function batteryPowerSummary({ onAc, percent }: { onAc: boolean | null; percent: number | null }) {
  const supply = onAc === null ? "供电状态未知" : onAc ? "当前插电" : "当前电池供电";
  return `${supply} · ${percent === null ? "电量未知" : `电量 ${percent}%`}`;
}

export function HotkeyEventPreviews({ profile, selected, draftActions }: Props) {
  const facts = profile?.facts, alert = facts?.batteryAlert;
  const has = (scope: HotkeyScope) => selected.has(scope);
  const low = has("batteryAlert") && !has("batteryLowBrightness") ? alert?.brightness : facts?.batteryLowBrightness;
  const brightness = low && (has("batteryLowBrightness") || has("batteryAlert") || low.recovery !== null) ? low : null;
  const action = has("batteryLowAction") ? facts?.batteryLowAction : null;
  const brightnessPower = brightness?.current ? batteryPowerSummary(brightness) : null;
  const actionPower = action?.current ? batteryPowerSummary(action) : null;
  const sharedPower = brightnessPower === actionPower ? brightnessPower : null;
  return <>
    {draftActions.includes("powerMode") && facts?.modeCycle && <ModeCycleFacts profile={facts.modeCycle} />}
    {facts?.gpuTurbo && (has("gpuTurbo") || facts.gpuTurbo.initial !== null) && <GpuTurboFacts profile={facts.gpuTurbo} />}
    {facts?.batteryThresholds && (has("batteryThresholds") || facts.batteryThresholds.recovery !== null) && <BatteryThresholdFacts profile={facts.batteryThresholds} />}
    {facts?.pdEvent && (has("powerDeliveryEvent") || facts.pdEvent.current?.initial != null) && <PdEventFacts profile={facts.pdEvent} />}
    {alert && (has("batteryAlert") || alert.codec.initial !== null) && <BatteryAlertFacts profile={alert} />}
    {sharedPower && <p className="ccc-control-caption">{sharedPower}</p>}
    {brightness && <BatteryLowBrightnessFacts profile={brightness} powerSummary={sharedPower ? null : brightnessPower}
      lowControl={alert?.binding?.lowControl} absolute={has("batteryAlert") && !has("batteryLowBrightness")} />}
    {action && <BatteryLowActionFacts profile={action} powerSummary={sharedPower ? null : actionPower} />}
    {facts?.lidAction && has("lidAction") && <SystemActionFacts profile={facts.lidAction} />}
  </>;
}
