import { invoke } from "./client";
import type { RgbColor } from "./lighting";

export interface MultiZoneCapabilities { keyboardType: number; lightBarSwitchable: boolean; lightBarEnabled: boolean; cover: boolean }
export interface MultiZoneValues {
  enabled: boolean; brightness: number; mode: number; speed: number;
  zones: [RgbColor, RgbColor, RgbColor, RgbColor]; lightBar: RgbColor | null; cover: RgbColor | null;
}
export interface MultiZoneTarget { capabilities: MultiZoneCapabilities; values: MultiZoneValues }
export interface MultiZoneProfile {
  revision: string; capabilities: MultiZoneCapabilities; current: MultiZoneValues | null;
  initial: MultiZoneValues | null; canRestore: boolean; unavailableReason: string | null; blockedReason: string | null;
}
export const readMultiZoneLighting = (): Promise<MultiZoneProfile | null> => invoke("read_multi_zone_lighting");
export const applyMultiZoneLighting = (expectedRevision: string, target: MultiZoneTarget): Promise<void> => invoke("apply_multi_zone_lighting", { request: { expectedRevision, target } });
export const restoreMultiZoneLighting = (expectedRevision: string): Promise<void> => invoke("restore_multi_zone_lighting", { expectedRevision });
