import { invoke } from "./client";

export interface TouchpadSnapshot { devices: string[]; enabled: boolean | null; saved: number }
export interface TouchpadTarget { devices: string[]; enabled: boolean; savedEnabled: boolean }
export interface TouchpadProfile {
  revision: string; current: TouchpadSnapshot; initial: TouchpadSnapshot | null;
  canRestore: boolean; restoreBlockedReason: string | null; blockedReason: string | null;
}
export interface TouchpadRequest { expectedRevision: string; enabled: boolean }
export const readTouchpad = (): Promise<TouchpadProfile> => invoke("read_touchpad");
export const applyTouchpad = (request: TouchpadRequest): Promise<void> => invoke("apply_touchpad", { request });
export const restoreTouchpad = (expectedRevision: string): Promise<void> => invoke("restore_touchpad", { expectedRevision });
