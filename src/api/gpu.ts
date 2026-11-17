/** GPU 频率偏移（对应 src-tauri/src/gpuctl；NVAPI 事务/钳位/回滚语义见该模块）。 */
import { invoke } from "./client";

export interface GpuTurboDevice { pciDevice: number; subsystem: number; revision: number; extended: number; bus: number; slot: number }
export interface GpuTurboOffsets { coreKhz: number | null; memKhz: number | null }
export interface GpuTurboLock { domain: number; mode: number; value: number; unknown1: number; unknown2: number; unknown3: number }
export interface GpuTurboTarget { lockMhz: number | null; offsets: { coreMhz: number; memoryMhz: number }[] | null }
export interface GpuTurboProfile {
  identity: string | null;
  current: {
    source: { mode: number; onAc: boolean | null; supplyError: string | null; capabilities: { limitClock: boolean; allModeOc: boolean };
      limits: { entertainmentMhz: number; performanceMhz: number; savingMhz: number; quietMhz: number; thermalMhz: number };
      saved: { coreMhz: number; memoryMhz: number }[] };
    devices: { id: GpuTurboDevice; offsets: GpuTurboOffsets; coreEditable: boolean; memoryEditable: boolean; coreRangeKhz: [number, number]; memoryRangeKhz: [number, number] }[];
    otherDevices: GpuTurboDevice[]; locks: [GpuTurboLock, GpuTurboLock]; baseMhz: number | null;
    off: GpuTurboTarget; offError: string | null; currentOn: GpuTurboTarget | null; onTargets: { mode: number; onAc: boolean; target: GpuTurboTarget; unavailableReason: string | null }[];
  } | null;
  error: string | null;
  initial: { devices: GpuTurboDevice[]; locks: [GpuTurboLock, GpuTurboLock] | null; offsets: GpuTurboOffsets[] } | null;
  restoreCurrent: { locks: [GpuTurboLock, GpuTurboLock] | null; offsets: GpuTurboOffsets[] } | null;
  restoreRevision: string | null; restoreError: string | null;
}

/** 应用写入前的原始基准（存在即可恢复）。 */
export interface GpuModeRawState { devices: GpuTurboDevice[]; locks: [GpuTurboLock, GpuTurboLock]; offsets: GpuTurboOffsets[] }
export type GpuModeSkipReason = "resume" | "quietPlus" | "notReady" | "unsupported";
export type GpuModeObservation = { status: "untracked" | "poweredOff" }
  | { status: "skipped"; value: GpuModeSkipReason } | { status: "unavailable"; value: string } | { status: "ready"; value: GpuModeRawState };
export interface GpuModeCandidate { decision: { target: GpuTurboTarget | null; skipped: GpuModeSkipReason | null } | null; unavailableReason: string | null; projected: GpuModeRawState | null }
type TurboCurrent = NonNullable<GpuTurboProfile["current"]>;
export interface GpuModePolicy {
  capabilities: TurboCurrent["source"]["capabilities"] | null; limits: TurboCurrent["source"]["limits"]; saved: TurboCurrent["source"]["saved"];
  gpuOc: boolean | null; simpleGpuOc: boolean; ready: boolean; gpuCount: number; cachedDeviceId: number;
}
export interface GpuModeDriver { identity: string; current: GpuModeRawState; devices: TurboCurrent["devices"] }
export interface GpuModeSelection { policy: GpuModePolicy; candidate: GpuModeCandidate; driver: GpuModeDriver | null; driverError: string | null; deferredReason: string | null }
export interface GpuModeCycle { policy: GpuModePolicy; choices: { mode: number; onAc: boolean; candidate: GpuModeCandidate }[]; driver: GpuModeDriver | null; driverError: string | null; deferredReason: string | null }
export interface GpuModeClockBaseline { origin: "beforeMode" | "afterEnable" | "poweredOff"; state: GpuModeRawState }

export interface GpuClockSettings {
  coreOffsetMhz: number | null;
  memOffsetMhz: number | null;
}

/** 读侧事实（read_gpu_profile 返回体）。 */
export interface GpuProfile {
  /** NVAPI 会话 + GPU 枚举 + P-States 读取全链路可用 */
  available: boolean;
  /** 不可用时的确切原因（阶段 + NVAPI 状态） */
  unavailableReason: string | null;
  gpuName: string | null;
  coreOffsetMhz: number | null;
  memOffsetMhz: number | null;
  coreEditable: boolean;
  memEditable: boolean;
  /** 有效区间（驱动 ∩ 钳位） */
  coreMinMhz: number;
  coreMaxMhz: number;
  memMinMhz: number;
  memMaxMhz: number;
  baseline: GpuClockSettings | null;
  warnings: string[];
}

/** 写侧事务结果。 */
export interface GpuApplyOutcome {
  ok: boolean;
  message: string;
  /** 回读确认的实际偏移（仅成功时；域未上报为 null） */
  coreOffsetMhz: number | null;
  memOffsetMhz: number | null;
  /** 回滚亦失败时的完整错误（硬件可能处于中间态） */
  rollbackError: string | null;
}

export const readGpuProfile = (): Promise<GpuProfile> => invoke("read_gpu_profile");

export const applyGpuClockOffsets = (
  coreMhz: number | null,
  memMhz: number | null,
): Promise<GpuApplyOutcome> => invoke("apply_gpu_clock_offsets", { coreMhz, memMhz });

export const restoreGpuBaseline = (): Promise<GpuApplyOutcome> => invoke("restore_gpu_baseline");
