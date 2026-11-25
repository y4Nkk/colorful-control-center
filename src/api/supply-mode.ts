import { invoke } from "./client";
import type { GpuPowerDevice, OemModeGpuAction, OemModeGpuState } from "./devices";
import type { GpuModeObservation, GpuModeCandidate, GpuModePolicy, GpuModeDriver, GpuModeClockBaseline } from "./gpu";
import type { OemEnergyState, OemModeTarget } from "./power";
import type { StoredFanCheckpoint } from "./fans";

export type SupplyModeTrigger = "startup" | "supplyChange" | "resume";
export interface SupplyConditions<T> {
  ac: T;
  battery: T;
  startupAc: T;
  startupBattery: T;
  resumeAc: T;
  resumeBattery: T;
}
export interface SupplyGpuCandidate { action: OemModeGpuAction | null; unavailableReason: string | null }
export interface SupplyClockReview extends SupplyConditions<GpuModeCandidate> {
  policy: GpuModePolicy;
  driver: GpuModeDriver | null;
  driverError: string | null;
  deferredReason: string | null;
}
export interface SupplyModeReview extends SupplyConditions<OemModeTarget | null> {
  deviceIdentity: string;
  identity: string;
  revision: string;
  currentMode: number;
  acMode: number;
  onAc: boolean | null;
  targets: OemModeTarget[];
  softwareFanActive: boolean;
  currentEnergy: OemEnergyState;
  dcModePower: boolean;
  gpu: SupplyConditions<SupplyGpuCandidate> & { supported: boolean; current: GpuPowerDevice | null };
  clocks: SupplyClockReview | null;
}
export interface SupplyModeRecovery {
  currentMode: number;
  initialMode: number;
  currentAcMode: number | null;
  initialAcMode: number | null;
  currentTurbo: boolean;
  initialTurbo: boolean;
  currentDtt: boolean;
  initialDtt: boolean;
  currentFan: StoredFanCheckpoint;
  initialFan: StoredFanCheckpoint;
  currentFanMode: number;
  initialFanMode: number;
  currentFanOffset: number | null;
  initialFanOffset: number | null;
  currentSoftwareFan: boolean;
  initialSoftwareFan: boolean;
  fanSavedSettingsChanged: boolean;
  currentSystem: [string, [number, number] | null] | null;
  initialSystem: [string, [number, number] | null] | null;
  planChanges: { scheme: string; current: [number, number]; initial: [number, number] }[];
  currentEnergy: OemEnergyState | null;
  initialEnergy: OemEnergyState | null;
  currentGpuPower: OemModeGpuState | null;
  initialGpuPower: OemModeGpuState | null;
  currentGpuClocks: GpuModeObservation | null;
  initialGpuClocks: GpuModeClockBaseline | null;
}
export interface SupplyModeStatus {
  revision: string;
  review: SupplyModeReview | null;
  enabled: boolean;
  canEnable: boolean;
  canRestore: boolean;
  recovery: SupplyModeRecovery | null;
  error: string | null;
  recoveryError: string | null;
  archiveRevision: string | null;
  listenerOnline: boolean;
  sleeping: boolean;
  active: boolean;
  canResume: boolean;
  issue: string | null;
  blockedReason: string | null;
  last: { atMs: number; trigger: SupplyModeTrigger; applied: boolean; previousMode: number | null;
    mode: number | null; onAc: boolean | null; error: string | null } | null;
}
export const readOemSupplyMode = (): Promise<SupplyModeStatus> => invoke("read_oem_supply_mode");
export const enableOemSupplyMode = (expectedRevision: string): Promise<void> => invoke("enable_oem_supply_mode", { expectedRevision });
export const resumeOemSupplyMode = (expectedRevision: string): Promise<void> => invoke("resume_oem_supply_mode", { expectedRevision });
export const pauseOemSupplyMode = (): Promise<void> => invoke("pause_oem_supply_mode");
export const restoreOemSupplyMode = (expectedRevision: string): Promise<void> => invoke("restore_oem_supply_mode", { expectedRevision });
export const archiveInvalidOemSupplyRecord = (expectedRevision: string): Promise<string> =>
  invoke("archive_invalid_oem_supply_record", { expectedRevision });
