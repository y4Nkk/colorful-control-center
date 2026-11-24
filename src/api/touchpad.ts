import { invoke } from "./client";

export interface TouchpadSnapshot { devices: string[]; enabled: boolean | null; saved: number }
export interface TouchpadTarget { devices: string[]; enabled: boolean; savedEnabled: boolean }
export interface TouchpadProfile { revision: string; current: TouchpadSnapshot; blockedReason: string | null }
export interface TouchpadRequest { expectedRevision: string; enabled: boolean }
export const readTouchpad = (): Promise<TouchpadProfile> => invoke("read_touchpad");
export const applyTouchpad = (request: TouchpadRequest): Promise<void> => invoke("apply_touchpad", { request });
