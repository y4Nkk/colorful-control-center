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
  /** CPU 温度（°C，65W 桶换算自 cmd12 buf[18] remote） */
  cpuTempC: number | null;
  /** cmd12 buf[18] 原始 remote（校准对照用） */
  cpuRemoteRaw: number | null;
  fan: FanSnapshot;
  nvidia: NvidiaSnapshot | null;
  sampledAtMs: number;
  warnings: string[];
}

export const getHardwareSnapshot = (): Promise<HardwareSnapshot> => invoke("read_hardware_snapshot");

export function onHardwareSnapshot(handler: (snapshot: HardwareSnapshot) => void): Promise<() => void> {
  return listenEvent<HardwareSnapshot>("hardware-snapshot", handler);
}
