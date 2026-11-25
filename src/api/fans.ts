/** 风扇控制（对应 src-tauri/src/fanctl；写入语义与钳位见该模块）。 */
import { invoke } from "./client";
import type { OpResult } from "./shared";

export type FanId = "cpu" | "gpu1" | "gpu2" | "sys";
export const FAN_LABELS: Record<FanId, string> = { cpu: "CPU", gpu1: "GPU 1", gpu2: "GPU 2", sys: "系统" };
export interface RpmPoint { tempC: number; rpmPct: number }
export interface RpmFan { id: FanId; maximumRpm: number; points: RpmPoint[] }
export interface SoftwarePoint { tempC: number; dutyPct: number }
export interface SoftwareCurve { points: SoftwarePoint[]; hysteresisC: number; rampPctPerS: number }
export type SoftwareCurves = Partial<Record<FanId, SoftwareCurve>>;
export interface StoredFanCheckpoint {
  software: SoftwareCurves | null;
  package: number[] | null;
  rpmPairs: { query: number; tables: [number[], number[]] }[] | null;
  fields: { offset: number; data: number[] }[];
  mode: number;
  offsetRaw: number | null;
}

export interface FanPointSet {
  id: "cpu" | "gpu1" | "gpu2";
  present: boolean;
  t1: number;
  d1Pct: number;
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}

export interface FanProfile {
  software: SoftwareCurves | null;
  curveKind: "twoPoint" | "rpmTable";
  rpmFans: RpmFan[];
  availableModes: number[];
  fanCount: number;
  customSupported: boolean;
  customBlockedReason: string | null;
  currentMode: number;
  offsetPct: number | null;
  offsetSlot: number | null;
  offsetBlockedReason: string | null;
  fans: FanPointSet[];
  canRestore: boolean;
  restoreRevision: string;
  warnings: string[];
  safetyStatus: string;
  blockedReason: string | null;
}

export interface CurvePointsDto {
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}


export const NOISELESS_FAN_LABEL = "寂静模式";
export const FAN_MODE_NOISELESS = 8;
export const CUSTOM_FAN_MODE = 6;
export const FAN_MODES: ReadonlyArray<{ id: number; label: string }> = [
  { id: 0, label: "自动" },
  { id: 1, label: "全速" },
  { id: 5, label: "能效优先" },
  { id: CUSTOM_FAN_MODE, label: "自定义" },
  { id: FAN_MODE_NOISELESS, label: NOISELESS_FAN_LABEL },
];

export function fanModeLabel(id: number) {
  if (id === 2) return "静音";
  if (id === 7) return "固件档位 7";
  return FAN_MODES.find(mode => mode.id === id)?.label ?? "模式未知";
}

export const readFanProfile = (): Promise<FanProfile> => invoke("read_fan_profile");
export const applyRpmFanCurves = (curves: Partial<Record<FanId, RpmPoint[]>>): Promise<OpResult> =>
  invoke("apply_rpm_fan_curves", { curves });
export const applyFanCurve = (
  cpu: CurvePointsDto | null,
  gpu1: CurvePointsDto | null,
  gpu2: CurvePointsDto | null,
): Promise<OpResult> => invoke("apply_fan_curve", { cpu, gpu1, gpu2 });
export const setFanMode = (mode: number): Promise<OpResult> => invoke("set_fan_mode", { mode });
export const setFanOffset = (expected: { mode: number; slot: number | null }, pct: number): Promise<OpResult> =>
  invoke("set_fan_offset", { expected, pct });
export type FanRestoreTarget = "initial" | "firmwareDefault" | "initialAndFirmwareDefault";
export const restoreFanSettings = (target: FanRestoreTarget, expectedRevision: string): Promise<OpResult> =>
  invoke("restore_fan_settings", { request: { target, expectedRevision } });

export interface DustSchedule { weekdays: number; hour: number; onResume: boolean }
export interface FanDustProfile {
  revision: string;
  running: boolean;
  current: DustSchedule | null;
  initial: { schedule: DustSchedule | null } | null;
  softwareActive: boolean;
  lastStart: "sent" | "uncertain" | "running" | "completed" | null;
  startBlockedReason: string | null;
  scheduleBlockedReason: string | null;
}
export const readFanDust = (): Promise<FanDustProfile | null> => invoke("read_fan_dust");
export const startFanDust = (expectedRevision: string): Promise<void> => invoke("start_fan_dust", { expectedRevision });
export const applyFanDustSchedule = (expectedRevision: string, schedule: DustSchedule): Promise<void> =>
  invoke("apply_fan_dust_schedule", { request: { expectedRevision, schedule } });
export const restoreFanDustSchedule = (expectedRevision: string): Promise<void> => invoke("restore_fan_dust_schedule", { expectedRevision });
