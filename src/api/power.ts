/** 性能调校（对应 src-tauri/src/powerctl；钳位与安全语义见该模块）。 */
import { invoke } from "./client";
import type { StoredFanCheckpoint } from "./fans";
import type { OpResult } from "./shared";
import type { GpuPowerDevice, OemModeGpuAction, OemModeGpuState } from "./devices";
import type { GpuModeSelection, GpuModeClockBaseline, GpuModeObservation } from "./gpu";


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
  restoreBlockedReason: string | null;
  blockedReason: string | null;
  currentMode: number;
  acMode: number;
  energy: OemEnergyState | null;
  restore: OemModeRestoreTarget | null;
  availableModes: number[];
  canRestore: boolean;
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
  fanMode: number;
  fanSavedMode: number;
  fanOffsetPct: number;
  aspm: [number, number] | null;
  powerPlan: number | null;
}
export type OemModeGpuReview = { status: "ready"; value: OemModeGpuTarget } | { status: "unavailable"; value: string };
export interface OemModeGpuTarget { nodeSupported: boolean; node: GpuPowerDevice | null; action: OemModeGpuAction | null; onAc: boolean | null; clocks: GpuModeSelection; stages: import("./devices").GpuPowerStages | null }
export interface OemModeCycleProfile {
  modes: OemModeTarget[];
  nodeSupported: boolean;
  node: GpuPowerDevice | null;
  nodeChoices: { mode: number; action: OemModeGpuAction | null; unavailableReason: string | null }[];
  clocks: import("./gpu").GpuModeCycle;
}
export interface OemModeGpuInitial { node: OemModeGpuState | null; clocks: GpuModeClockBaseline | null; driverIdentity: string | null }
export interface OemModeRestoreTarget {
  revision: string;
  mode: number;
  acMode: number | null;
  energy: OemEnergyState | null;
  gpu: OemModeGpuInitial | null;
  currentGpuPower: GpuPowerDevice | null;
  currentGpuClocks: GpuModeObservation;
  currentGpuDriverIdentity: string | null;
  turbo: boolean;
  dtt: boolean;
  entertainmentQuiet: boolean | null;
  fan: StoredFanCheckpoint;
  system: [string, [number, number] | null] | null;
  systemPlans: { scheme: string; initial: [number, number]; current: [number, number] | null }[];
}
export const OEM_POWER_MODES = ["安静", "省电", "性能", "娱乐", "静音增强"] as const;
export interface FnKeyModeConfirmation { identity: { pid: number; creationTime: [number, number] }; modeRevision: string }
export const readOemPowerMode = (): Promise<OemPowerMode> => invoke("read_oem_power_mode");
export const applyOemPowerMode = (expected: OemModeTarget, confirmedFnKey: FnKeyModeConfirmation | null = null):
  Promise<OpResult | { confirmFnKey: FnKeyModeConfirmation }> => invoke("apply_oem_power_mode", { expected, confirmedFnKey });
export const restoreOemPowerMode = (expectedRevision: string): Promise<OpResult> => invoke("restore_oem_power_mode", { expectedRevision });
export interface OemPerformanceValues { turboFan: boolean | null; dtt: boolean | null }
export const PERFORMANCE_OPTION_LABELS = { turboFan: "Turbo 风扇", dtt: "动态调校 DTT" } as const satisfies Record<keyof OemPerformanceValues, string>;
export interface OemPerformanceDraft extends OemPerformanceValues { quietFan: boolean | null }
export interface OemPerformanceOptions {
  restoreBlockedReason: string | null;
  acMode: number | null;
  energy: OemEnergyState | null;
  restore: OemModeRestoreTarget | null;
  blockedReason: string | null;
  current: OemPerformanceValues; currentMode: number; customFan: boolean; softwareActive: boolean;
  appliesInCurrentMode: boolean;
  entertainmentOptionsPending: boolean;
  quietFan: { enabled: boolean; active: boolean; remembered: boolean } | null;
  dttDriverAvailable: boolean | null; revision: string; canRestore: boolean; warnings: string[];
}
export const readOemPerformanceOptions = () => invoke<OemPerformanceOptions>("read_oem_performance_options");
export const applyOemPerformanceOptions = (expectedRevision: string, { quietFan, ...values }: OemPerformanceDraft, replaceCustomFan = false) =>
  invoke<OpResult>("apply_oem_performance_options", { request: { expectedRevision, values, quietFan, replaceCustomFan } });
export interface CpuControls {
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
}
export const applySavedCpu = (controls: CpuControls): Promise<OpResult> => invoke("apply_saved_cpu", { controls });
export const applyRuntimeCpu = (controls: CpuControls): Promise<OpResult> => invoke("apply_runtime_cpu", { controls });
export const restorePowerProfile = (): Promise<OpResult> => invoke("restore_power_profile");
