/** 性能调校（对应 src-tauri/src/powerctl；钳位与安全语义见该模块）。 */
import { invoke } from "./client";
import type { OpResult } from "./shared";
import type { FnKeyIdentity } from "./oem-owner";
import type { GpuPowerDevice, OemModeGpuAction } from "./devices";
import type { GpuModeSelection } from "./gpu";

export interface WindowsPowerPlan { id: string; name: string }
export interface WindowsPowerPlans {
  revision: string;
  activeId: string;
  plans: WindowsPowerPlan[];
  blockedReason: string | null;
}

export const readWindowsPowerPlans = () => invoke<WindowsPowerPlans>("read_windows_power_plans");
export const applyWindowsPowerPlan = (expectedRevision: string, targetId: string) =>
  invoke<void>("apply_windows_power_plan", { request: { expectedRevision, targetId } });

export interface PowerValues {
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
}
export interface RuntimeCpuSettings {
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetUnits: number | null;
  gtOffsetUnits: number | null;
}
export const offsetUnitsToMv = (units: number): number => units / 1.024;
export interface PowerProfile {
  pl1W: number | null;
  pl2W: number | null;
  plTimeS: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
  defaultPl1W: number | null;
  defaultPl2W: number | null;
  defaultCoreOffsetMv: number | null;
  msrRuntimeAvailable: boolean;
  msrRuntimeError: string | null;
  canRestore: boolean;
  runtime: PowerValues;
  runtimeWarnings: string[];
  warnings: string[];
}

export const readPowerProfile = (): Promise<PowerProfile> => invoke("read_power_profile");
export interface OemEnergyState { hardwarePState: boolean; energyStar: number | null }
export interface OemPowerMode {
  blockedReason: string | null;
  currentMode: number;
  acMode: number;
  energy: OemEnergyState | null;
  availableModes: number[];
  targets: OemModeTarget[];
  softwareActive: boolean;
}
export interface OemModeTarget {
  mode: number;
  fromMode: number | null;
  gpu: OemModeGpuReview | null;
  revision: string | null;
  unavailableReason: string | null;
  acMode: number | null;
  energy: OemEnergyState | null;
  turbo: boolean;
  dtt: boolean;
  fan: OemModeFanPolicy;
  aspm: [number, number] | null;
  powerPlan: number | null;
}
export type OemModeFanPolicy =
  | { control: "firmware"; savedMode: number }
  | { control: "explicit"; mode: number; savedMode: number; offsetPct: number | null };
export type OemModeGpuReview = { status: "ready"; value: OemModeGpuTarget } | { status: "unavailable"; value: string };
export interface OemModeGpuTarget { nodeSupported: boolean; node: GpuPowerDevice | null; action: OemModeGpuAction | null; onAc: boolean | null; clocks: GpuModeSelection; stages: import("./devices").GpuPowerStages | null }
export interface OemModeCycleProfile {
  modes: OemModeTarget[];
  nodeSupported: boolean;
  node: GpuPowerDevice | null;
  nodeChoices: { mode: number; action: OemModeGpuAction | null; unavailableReason: string | null }[];
  clocks: import("./gpu").GpuModeCycle;
}
export const OEM_POWER_MODES = ["安静", "省电", "性能", "娱乐"] as const;
export interface FnKeyModeConfirmation { identity: FnKeyIdentity; modeRevision: string }
export const readOemPowerMode = (): Promise<OemPowerMode> => invoke("read_oem_power_mode");
export const applyOemPowerMode = (expected: OemModeTarget, confirmedFnKey: FnKeyModeConfirmation | null = null):
  Promise<OpResult | { confirmFnKey: FnKeyModeConfirmation }> => invoke("apply_oem_power_mode", { expected, confirmedFnKey });
export interface OemPerformanceValues { turboFan: boolean | null; dtt: boolean | null }
export const PERFORMANCE_OPTION_LABELS = { turboFan: "Turbo 风扇", dtt: "动态调校 DTT" } as const satisfies Record<keyof OemPerformanceValues, string>;
export interface OemPerformanceChange {
  values: OemPerformanceValues;
  quietFan: "current" | "entertainmentOn" | "entertainmentOff" | null;
  replaceCustomFan: boolean;
}
export interface OemPerformanceOptions {
  blockedReason: string | null;
  current: OemPerformanceValues; currentMode: number;
  turboActive: boolean; dttActive: boolean;
  appliesInCurrentMode: boolean;
  quietFan: { enabled: boolean; active: boolean; remembered: boolean } | null;
  dttDriverAvailable: boolean | null; revision: string; warnings: string[];
}
export const readOemPerformanceOptions = () => invoke<OemPerformanceOptions>("read_oem_performance_options");
export const applyOemPerformanceOptions = (expectedRevision: string, change: OemPerformanceChange) =>
  invoke<OpResult>("apply_oem_performance_options", { request: { expectedRevision, ...change } });
export interface CpuControls {
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
}
export const applySavedCpu = (controls: CpuControls): Promise<OpResult> => invoke("apply_saved_cpu", { controls });
export const applyRuntimeCpu = (controls: CpuControls): Promise<OpResult> => invoke("apply_runtime_cpu", { controls });
export const restorePowerProfile = (): Promise<OpResult> => invoke("restore_power_profile");
