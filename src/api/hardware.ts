/** 硬件监控（对应 src-tauri/src/hal/；1Hz hardware-snapshot 事件推送）。 */
import { invoke, listenEvent } from "./client";

export interface Reading {
  ok: boolean;
  value: number;
  /** 回声检测命中（值恒为命令号），不可信 */
  stale: boolean;
}

export interface FanSnapshot {
  cpuRpm: number | null;
  cpuDutyPct: number | null;
  gpuRpm: number | null;
  gpuDutyPct: number | null;
}

export interface NvidiaSnapshot {
  name: string;
  tempC: number | null;
  fanPct: number | null;
  utilizationPct: number | null;
  memUsedMb: number | null;
  memTotalMb: number | null;
}

export interface HardwareSnapshot {
  dchuAvailable: boolean;
  cpuTempC: Reading | null;
  cpuFreqMhz: Reading | null;
  cpuFreqMaxMhz: Reading | null;
  gpuTempC: Reading | null;
  gpuFreqMhz: Reading | null;
  fan: FanSnapshot;
  memoryUsedMb: Reading | null;
  nvidia: NvidiaSnapshot | null;
  sampledAtMs: number;
  warnings: string[];
}

export const getHardwareSnapshot = (): Promise<HardwareSnapshot> => invoke("read_hardware_snapshot");

export function onHardwareSnapshot(handler: (snapshot: HardwareSnapshot) => void): Promise<() => void> {
  return listenEvent<HardwareSnapshot>("hardware-snapshot", handler);
}
