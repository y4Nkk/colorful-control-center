import type { HotkeyProfile, HotkeyScope } from "../../api/client";
import type { useHotkeySettings } from "./useHotkeySettings";
import { GpuTurboFacts } from "./GpuTurboFacts";
import { BatteryThresholdFacts } from "./BatteryThresholdFacts";
import { PdEventFacts } from "./PdEventFacts";
import { BatteryAlertFacts } from "./BatteryAlertFacts";
import { BatteryLowBrightnessFacts } from "./BatteryLowBrightnessFacts";
import { SystemActionFacts } from "./SystemActionFacts";
import { BatteryLowActionFacts } from "./BatteryLowActionFacts";
import { ModeCycleFacts } from "./ModeCycleFacts";

type Props = { profile: HotkeyProfile | null; selected: ReadonlySet<HotkeyScope>; draftActions: HotkeyScope[]; busy: boolean; run: ReturnType<typeof useHotkeySettings>["run"] };
export function HotkeyEventPreviews({ profile, selected, draftActions, busy, run }: Props) {
  const facts = profile?.facts, alert = facts?.batteryAlert;
  const has = (scope: HotkeyScope) => selected.has(scope);
  const low = has("batteryAlert") && !has("batteryLowBrightness") ? alert?.brightness : facts?.batteryLowBrightness;
  return <>
    {draftActions.includes("powerMode") && facts?.modeCycle && <ModeCycleFacts profile={facts.modeCycle} />}
    {facts?.gpuTurbo && (has("gpuTurbo") || facts.gpuTurbo.initial !== null) && <GpuTurboFacts profile={facts.gpuTurbo} busy={busy} run={run} />}
    {facts?.batteryThresholds && (has("batteryThresholds") || facts.batteryThresholds.recovery !== null) && <BatteryThresholdFacts profile={facts.batteryThresholds} busy={busy} run={run} />}
    {facts?.pdEvent && (has("powerDeliveryEvent") || facts.pdEvent.current?.initial != null) && <PdEventFacts profile={facts.pdEvent} busy={busy} run={run} />}
    {alert && (has("batteryAlert") || alert.codec.initial !== null) && <BatteryAlertFacts profile={alert} busy={busy} run={run} />}
    {low && (has("batteryLowBrightness") || has("batteryAlert") || low.recovery !== null) && <BatteryLowBrightnessFacts profile={low} lowControl={alert?.binding?.lowControl} absolute={has("batteryAlert") && !has("batteryLowBrightness")} busy={busy} run={run} />}
    {facts?.batteryLowAction && has("batteryLowAction") && <BatteryLowActionFacts profile={facts.batteryLowAction} />}
    {facts?.lidAction && has("lidAction") && <SystemActionFacts profile={facts.lidAction} />}
  </>;
}
