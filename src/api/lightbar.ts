import { invoke, type RgbColor } from "./client";

export type X170LightBarEffect = "static" | "breath" | "blink" | "cycle" | "meteor" | "wave" | "ripple" | "random";
export interface X170LightBarSettings { enabled: boolean; brightness: number; speed: number; effect: X170LightBarEffect; color: RgbColor | null }
export type X58LightBarEffect = "static" | "breath" | "wave" | "cycle" | "scan" | "colorWave";
export interface X58LightBarSettings { brightness: number; speed: number; effect: X58LightBarEffect; colorId: number | null }
export type LightBarValues = { kind: "x170"; settings: X170LightBarSettings } | { kind: "x58"; settings: X58LightBarSettings };
export interface LightBarProfile {
  kind: "x170" | "x58";
  revision: string;
  current: LightBarValues | null;
  presets: LightBarValues[];
  initial: LightBarValues | null;
  blockedReason: string | null;
  warnings: string[];
}
export const readLightBar = (): Promise<LightBarProfile | null> => invoke("read_light_bar");
export const applyLightBar = (expectedRevision: string, values: LightBarValues): Promise<void> => invoke("apply_light_bar", { request: { expectedRevision, values } });
export const restoreLightBar = (expectedRevision: string): Promise<void> => invoke("restore_light_bar", { expectedRevision });
