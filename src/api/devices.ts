import { invoke, listenEvent } from "./client";
export interface DisplayMode {
  device: string; identity: string; name: string; width: number; height: number; bits: number;
  orientation: number; x: number; y: number; flags: number; frequency: number;
}
export interface DisplayProfile {
  displays: { current: DisplayMode; frequencies: number[]; primary: boolean }[];
  rotation: { before: DisplayMode; target: DisplayMode } | null;
  pending: { before: DisplayMode[]; targets: DisplayMode[]; rotation: boolean; automatic: boolean; trialId: number | null; remainingMs: number | null; profileSession: number | null; error: string | null } | null;
  error: string | null; blockedReason: string | null;
}
export type DisplayPending = NonNullable<DisplayProfile["pending"]>;
export type OemModeGpuValues = Pick<GpuPowerDevice, "enabled" | "automaticPowerOff">;
export interface OemModeGpuAction { stage: "beforeMode" | "afterMode"; target: OemModeGpuValues }
export interface OemModeGpuState { id: string; name: string; target: OemModeGpuValues }
export interface GpuPowerStage { expected: GpuPowerDevice; target: OemModeGpuValues; restore: boolean }
export interface GpuPowerStages { before: GpuPowerDevice; mode: number | null; preparation: GpuPowerStage | null; beforeMode: GpuPowerStage | null; afterMode: GpuPowerStage | null; finish: GpuPowerStage | null }
export interface AutomaticRefreshFacts {
  identity: string; display: DisplayMode; candidates: number[]; low: number; savedOrHighest: number;
  fields: [number, number, number]; mode: number; modes: number[]; customer: number; product: number; supplyModeSwitch: boolean;
  onAc: boolean | null; defaultEnabled: boolean;
}
export interface AutomaticRefreshStatus {
  revision: string; current: AutomaticRefreshFacts | null; initial: [number, number, number] | null;
  initialDisplay: DisplayMode | null; preview: { before: DisplayMode; target: DisplayMode } | null;
  active: boolean; canStart: boolean; canRestore: boolean; error: string | null;
  listenerOnline: boolean; sleeping: boolean; blockedReason: string | null; issue: string | null;
}
export interface GpuPowerDevice { id: string; name: string; enabled: boolean; automaticPowerOff: boolean; problem: number; drivesDisplay: boolean; canDisable: boolean }
export interface GpuPowerProfile { devices: GpuPowerDevice[] }
export type GraphicsModeVerification = "uncertain" | "pendingReboot" | "confirmed" | "notApplied";
export interface GraphicsModeProfile {
  current: { mode: number; availableMask: number } | null;
  revision: string;
  lastRequest: { target: number; verification: GraphicsModeVerification } | null;
}
export const readGraphicsMode = (): Promise<GraphicsModeProfile> => invoke("read_graphics_mode");
export const applyGraphicsMode = (expectedRevision: string, target: number): Promise<void> => invoke("apply_graphics_mode", { request: { expectedRevision, target } });
export const rebootForGraphicsMode = (expectedRevision: string): Promise<void> => invoke("reboot_for_graphics_mode", { expectedRevision });
export const readDisplayModes = (): Promise<DisplayProfile> => invoke("read_display_modes");
export const readAutomaticDisplayTrial = (): Promise<DisplayPending | null> => invoke("read_automatic_display_trial");
export const readAutomaticRefresh = (): Promise<AutomaticRefreshStatus> => invoke("read_automatic_refresh");
export const enableAutomaticRefresh = (expectedRevision: string): Promise<void> => invoke("enable_automatic_refresh", { expectedRevision });
export const restoreAutomaticRefresh = (expectedRevision: string): Promise<void> => invoke("restore_automatic_refresh", { expectedRevision });
export const onAutomaticDisplayTrial = (handler: () => void): Promise<() => void> => listenEvent("automatic-display-trial", handler);
export const applyDisplayRefresh = (expected: DisplayMode, frequency: number): Promise<void> => invoke("apply_display_refresh", { expected, frequency });
export const applyDisplayRotation = (expected: DisplayMode): Promise<void> => invoke("apply_display_rotation", { expected });
export const keepDisplayMode = (trialId: number, expected: DisplayMode): Promise<void> => invoke("keep_display_mode", { trialId, expected });
export const cancelDisplayTrial = (trialId: number): Promise<void> => invoke("cancel_display_trial", { trialId });
export const readGpuPower = (): Promise<GpuPowerProfile> => invoke("read_gpu_power");
export const applyGpuPower = (expected: GpuPowerDevice, enabled: boolean): Promise<void> => invoke("apply_gpu_power", { expected, enabled });

export type PdEncoding = "legacyZeroBased" | "legacyOneBased" | "adapterV1";
export interface PowerDeliveryTarget {
  level: number; encoding: PdEncoding; adapterWatts: number | null; availableLevels: number[];
}
export interface PowerDeliveryProfile {
  revision: string;
  encoding: PdEncoding;
  currentLevel: number;
  currentValid: boolean;
  availableLevels: number[];
  zeroBased: boolean;
  adapterWatts: number | null;
  initial: { value: number; zeroBased: boolean } | null;
  restoreBlockedReason: string | null;
  blockedReason: string | null;
}
export interface PlatformPowerProfile { powerDelivery: PowerDeliveryProfile | null; deepS5: boolean | null; warnings: string[] }
export const readPlatformPower = (): Promise<PlatformPowerProfile> => invoke("read_platform_power");
export const applyPdLevel = (expectedRevision: string, level: number): Promise<void> => invoke("apply_pd_level", { request: { expectedRevision, level } });
export const restorePdLevel = (expectedRevision: string): Promise<void> => invoke("restore_pd_level", { expectedRevision });
