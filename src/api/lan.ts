import { invoke } from "./client";
export type LanField = "powerSavingMode" | "pnpCapabilities" | "wakeOnLink" | "s5WakeOnLan" | "shutdownWake" | "wakeOnSlot";
export interface LanValue { kind: number; bytes: number[] }
export interface LanIdentity { instance: string; driver: string; netCfgId: LanValue | null; driverVersion: LanValue | null; matchingId: LanValue | null }
export interface LanDevice { identity: LanIdentity; name: string; values: Record<LanField, LanValue | null> }
export interface LanInitial { identity: LanIdentity; fields: Partial<Record<LanField, LanValue | null>> }
export interface LanProfile {
  revision: string;
  frame: { capabilities: { powerSaving: boolean; wakePatch: boolean }; recommendation: boolean | null; devices: LanDevice[] };
  initial: LanInitial[]; lastError: string | null; blockedReason: string | null;
}
export interface LanRequest { expectedRevision: string; deviceId: string; powerSaving: boolean | null; applyWakePatch: boolean }
export interface LanChange { field: LanField; before: LanValue | null; target: LanValue | null }
export type LanWritableField = "powerSavingMode" | "pnpCapabilities" | "wakeOnLink";
export interface LanPolicyTarget {
  capabilities: LanProfile["frame"]["capabilities"];
  devices: { identity: LanIdentity; name: string; values: Partial<Record<LanWritableField, LanValue | null>> }[];
}
export const readLanPolicy = () => invoke<LanProfile | null>("read_lan_policy");
export const previewLanPolicy = (request: LanRequest) => invoke<LanChange[]>("preview_lan_policy", { request });
export const applyLanPolicy = (request: LanRequest) => invoke<void>("apply_lan_policy", { request });
export const restoreLanPolicy = (expectedRevision: string, deviceId: string) => invoke<void>("restore_lan_policy", { expectedRevision, deviceId });
