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
  graphicsClockMhz: number | null;
  memoryClockMhz: number | null;
  powerDrawW: number | null;
  powerLimitW: number | null;
  cudaCores: number | null;
  memoryBusWidthBits: number | null;
}

export interface OemMemoryReading {
  reportedClockMhz: number | null;
  referenceClockMhz: number | null;
}

export interface HardwareSnapshot {
  dchuAvailable: boolean;
  oemPowerMode: number | null;
  /** CPU 温度（°C，本机原厂零 TDP 分支直接使用 cmd12 buf[18]）。 */
  cpuTempC: number | null;
  /** cmd12 buf[18] 原始 remote（校准对照用） */
  cpuRemoteRaw: number | null;
  /** Windows处理器效用（限制到100%）与实时频率（标称基频 × 性能百分比），同一次采样；包含独立采样时间。 */
  cpuUtility: { valuePct: number; clockMhz: number | null; sampledAtMs: number } | null;
  fan: FanSnapshot;
  nvidia: NvidiaSnapshot | null;
  /** 原厂共享报告，未报告或本次 OEM 传输失败时为 null。 */
  oemMemory: OemMemoryReading | null;
  nvidiaSuspended: boolean;
  sampledAtMs: number;
  warnings: string[];
}

export const getHardwareSnapshot = (): Promise<HardwareSnapshot> => invoke("read_hardware_snapshot");

export function onHardwareSnapshot(handler: (snapshot: HardwareSnapshot) => void): Promise<() => void> {
  return listenEvent<HardwareSnapshot>("hardware-snapshot", handler);
}

export interface HardwareCapacity {
  totalBytes: number;
  availableBytes: number;
}

export interface SystemOverview {
  cpu: { vendor: "intel" | "amd" | "unknown"; name: string | null;
    physicalCores: number | null; logicalProcessors: number | null;
    baseClockMhz: number | null; maxClockMhz: number | null };
  memory: HardwareCapacity | null;
  /** SMBIOS Type 17 已安装内存条共同报告的配置速率（MT/s）。 */
  memorySpeedMts: number | null;
  volumes: { mount: string; label: string | null; fileSystem: string | null; capacity: HardwareCapacity | null }[];
  warnings: string[];
}

export const readSystemOverview = (): Promise<SystemOverview> => invoke("read_system_overview");
