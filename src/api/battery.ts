import { invoke } from "./client";

export interface BatteryChargeValues { enabled: boolean; start: number; stop: number }
export interface BatteryChargeState { current: BatteryChargeValues; startValues: number[]; stopValues: number[] }
export interface BatteryChargeTarget { values: BatteryChargeValues; startValues: number[]; stopValues: number[] }
export interface BatteryPowerStatus {
  present: boolean | null; acOnline: boolean | null; charging: boolean | null;
  percent: number | null; remainingSeconds: number | null; windowsBatterySaver: boolean | null;
}
export interface BatteryDiagnostics {
  version: number; manufacturedOn: string | null; cycleCount: number; fullCapacityRaw: number; designCapacityRaw: number;
  capacityPct: number | null; batteryStatus: number; permanentFailureStatus: number; operationStatus: number; stopChargingThreshold: number;
}
export interface BatteryProfile {
  charge: BatteryChargeState | null; revision: string | null; power: BatteryPowerStatus | null;
  diagnostics: BatteryDiagnostics | null; canRestore: boolean; warnings: string[];
  chargeLockReason: string | null;
}
export const readBatteryProfile = () => invoke<BatteryProfile>("read_battery_profile");
export const applyBatteryCharge = (expectedRevision: string, values: BatteryChargeValues) =>
  invoke<void>("apply_battery_charge", { request: { expectedRevision, values } });
export const restoreBatteryCharge = (expectedRevision: string) => invoke<void>("restore_battery_charge", { expectedRevision });

export const ENERGY_POLICY_LABEL = "电池节能计划";
export interface EnergyLimits { floor: number; ceiling: number }
export interface EnergySlot { start: number; end: number }
export interface EnergyPolicy {
  enabled: boolean; everyDay: boolean; weekdays: number; slots: [EnergySlot | null, EnergySlot | null];
  automatic: boolean; limits: EnergyLimits;
}
export interface EnergyPolicyProfile {
  saved: EnergyPolicy | null; capability: { factory: EnergyLimits; factoryDefaulted: boolean }; revision: string;
  source: "application" | "oemSaved" | "unconfigured"; canRestore: boolean; recoveryRequired: boolean; chargeOnStop: BatteryChargeState;
  error: string | null; canWrite: boolean;
  chargeReported: BatteryChargeValues;
  hasInitialPlan: boolean; initialCharge: BatteryChargeValues;
}
export interface BatteryEnergyTarget { capability: EnergyPolicyProfile["capability"]; policy: EnergyPolicy }
export const readEnergyPolicy = () => invoke<EnergyPolicyProfile | null>("read_energy_policy");
export const applyEnergyPolicy = (expectedRevision: string, policy: EnergyPolicy) => invoke<void>("apply_energy_policy", { request: { expectedRevision, policy } });
export const restoreEnergyPolicy = (expectedRevision: string) => invoke<void>("restore_energy_policy", { expectedRevision });

export type CalibrationPhase = "discharge1" | "charge1" | "discharge2" | "charge2" | "adjusting";
export type CalibrationStatus = "running" | "paused" | "completed" | "stopped" | "failed" | "recoveryRequired";
export interface CalibrationState {
  status: CalibrationStatus; phase: CalibrationPhase; elapsedMs: number; phaseElapsedMs: number; error: string | null;
  facts: { present: boolean | null; acOnline: boolean | null; percent: number | null;
    batteryStatus: number; permanentFailure: number; stopThreshold: number };
}
export interface BatteryCalibrationProfile {
  revision: string; task: CalibrationState | null; workerRunning: boolean; controlAcquired: boolean; canStart: boolean; canStop: boolean;
  recoveryRequired: boolean; stopThreshold: number; restoreCharge: BatteryChargeValues;
  restoreEnergyEnabled: boolean | null; knownEnergyPlan: boolean; hasEnergyCapability: boolean;
  unavailableReason: string | null; error: string | null;
}
export const readBatteryCalibration = () => invoke<BatteryCalibrationProfile | null>("read_battery_calibration");
export const startBatteryCalibration = (expectedRevision: string) => invoke<void>("start_battery_calibration", { expectedRevision });
export const stopBatteryCalibration = (expectedRevision: string) => invoke<void>("stop_battery_calibration", { expectedRevision });
export const restoreBatteryCalibration = (expectedRevision: string) => invoke<void>("restore_battery_calibration", { expectedRevision });
