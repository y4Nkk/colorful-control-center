/** 风扇控制（对应 src-tauri/src/fanctl；写入语义与钳位见该模块）。 */
import { invoke } from "./client";

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
  fanCount: number;
  customSupported: boolean;
  currentMode: number;
  offsetPct: number;
  fans: FanPointSet[];
}

export interface CurvePointsDto {
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}

export interface OpResult {
  ok: boolean;
  message: string;
}

export const FAN_MODES: ReadonlyArray<{ id: number; label: string }> = [
  { id: 0, label: "自动" },
  { id: 1, label: "全速" },
  { id: 5, label: "MaxQ" },
  { id: 6, label: "自定义" },
  { id: 8, label: "静音" },
];

export const readFanProfile = (): Promise<FanProfile> => invoke("read_fan_profile");
export const applyFanCurve = (
  cpu: CurvePointsDto | null,
  gpu1: CurvePointsDto | null,
  gpu2: CurvePointsDto | null,
): Promise<OpResult> => invoke("apply_fan_curve", { cpu, gpu1, gpu2 });
export const setFanMode = (mode: number): Promise<OpResult> => invoke("set_fan_mode", { mode });
export const setFanOffset = (pct: number): Promise<OpResult> => invoke("set_fan_offset", { pct });
export const restoreFanDefault = (): Promise<OpResult> => invoke("restore_fan_default");
