/** 性能调校（对应 src-tauri/src/powerctl；钳位与安全语义见该模块）。 */
import { invoke } from "./client";

export interface Reading2 {
  ok: boolean;
  message: string;
}

export interface PowerProfile {
  pl1W: number | null;
  pl2W: number | null;
  plTimeS: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
  defaultPl1W: number | null;
  defaultPl2W: number | null;
  defaultCoreOffsetMv: number | null;
  msrRuntimeAvailable: boolean;
  warnings: string[];
}

export const readPowerProfile = (): Promise<PowerProfile> => invoke("read_power_profile");
export const applyPlPersist = (pl1W: number | null, pl2W: number | null): Promise<Reading2> =>
  invoke("apply_pl_persist", { pl1W, pl2W });
export const applyVoltagePersist = (
  coreMv: number | null,
  gtMv: number | null,
): Promise<Reading2> => invoke("apply_voltage_persist", { coreMv, gtMv });
export const applyPlRuntime = (pl1W: number | null, pl2W: number | null): Promise<Reading2> =>
  invoke("apply_pl_runtime", { pl1W, pl2W });
export const applyVoltageRuntime = (plane: number, mv: number): Promise<Reading2> =>
  invoke("apply_voltage_runtime", { args: { plane, mv } });
