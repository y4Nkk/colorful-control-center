/** 主窗口按需采样，通过 hardware-snapshot 接收事件。 */
import { invoke, listenEvent } from "./client";
import type { FanId } from "./fans";

export interface Reading {
  ok: boolean;
  value: number;
  /** 回声检测命中（值恒为命令号），不可信 */
  stale: boolean;
}

export interface FanReading {
  rpm: number | null;
  dutyPct: number | null;
  /** 对应风扇通道的 EC 温度（°C）。 */
  tempC: number | null;
}

export interface NvidiaSnapshot {
  connectionId: number;
  name: string;
  tempC: number | null;
  utilizationPct: number | null;
  memoryUsedPct: number | null;
  memTotalMb: number | null;
  graphicsClockMhz: number | null;
  memoryClockMhz: number | null;
  powerDrawW: number | null;
  powerLimitW: number | null;
  cudaCores: number | null;
  memoryBusWidthBits: number | null;
}

export type GpuClockLimitReason = "idle" | "applicationClocks" | "powerCap" | "hardwareSlowdown"
  | "syncBoost" | "softwareTemperature" | "hardwareTemperature" | "powerBrake" | "displayClocks" | "unknown";

export interface GpuDetails {
  encoderUtilizationPct: number | null;
  decoderUtilizationPct: number | null;
  performanceState: number | null;
  pcieLinkGeneration: number | null;
  pcieLinkWidth: number | null;
  pcieMaxLinkGeneration: number | null;
  pcieMaxLinkWidth: number | null;
  clockLimitReasons: GpuClockLimitReason[] | null;
  memoryTemperatureC: number | null;
  fanTargetRpm: number | null;
}

export interface CpuUtilityReading {
  valuePct: number;
  clockMhz: number | null;
  maxActiveClockMhz: number | null;
  sampledAtMs: number;
}

export interface OemMemoryReading {
  reportedClockMhz: number | null;
  referenceClockMhz: number | null;
}

export interface HardwareSnapshot {
  dchuAvailable: boolean;
  oemPowerMode: number | null;
  /** Windows 处理器效用与同一窗口内的活跃逻辑处理器频率估算。 */
  cpuUtility: CpuUtilityReading | null;
  fan: Record<FanId, FanReading>;
  nvidia: NvidiaSnapshot | null;
  /** 原厂共享报告，未报告或本次 OEM 传输失败时为 null。 */
  oemMemory: OemMemoryReading | null;
  nvidiaSuspended: boolean;
  sampledAtMs: number;
  warnings: string[];
}

export const updateHardwareMonitoring = (active: boolean): Promise<void> => invoke("update_hardware_monitoring", { active });

export function onHardwareSnapshot(handler: (snapshot: HardwareSnapshot) => void): Promise<() => void> {
  return listenEvent<HardwareSnapshot>("hardware-snapshot", handler);
}

export interface HardwareCapacity {
  totalBytes: number;
  availableBytes: number;
}

export interface MemoryModule {
  locator: string | null;
  capacityBytes: number | null;
  memoryType: string | null;
  manufacturer: string | null;
  partNumber: string | null;
  configuredSpeedMts: number | null;
  soldered: boolean | null;
}

export interface SystemInventory {
  cpu: { vendor: "intel" | "amd" | "unknown"; name: string | null;
    physicalCores: number | null; logicalProcessors: number | null;
    baseClockMhz: number | null; maxClockMhz: number | null };
  /** SMBIOS Type 17 已安装内存条共同报告的配置速率（MT/s）。 */
  memorySpeedMts: number | null;
  memoryModules: MemoryModule[] | null;
  memorySlots: { used: number; total: number } | null;
  volumes: { mount: string; label: string | null; fileSystem: string | null; capacity: HardwareCapacity | null }[] | null;
  warnings: string[];
}

export const readMemoryCapacity = (): Promise<HardwareCapacity> => invoke("read_memory_capacity");
export const readSystemInventory = (refreshVolumeMetadata: boolean): Promise<SystemInventory> =>
  invoke("read_system_inventory", { refreshVolumeMetadata });
export const readGpuDetails = (connectionId: number): Promise<GpuDetails | null> =>
  invoke("read_gpu_details", { connectionId });
