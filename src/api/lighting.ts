/** 键盘灯契约由 lighting/protocol.rs 与 docs/keyboard-contract.md 拥有。 */
import { invoke } from "./client";

export type KeyboardFamily = "zoneRgb" | "white" | "perkey" | "multiZone" | "none" | "unknown";
export type KeyboardEffect = "random" | "zones" | "breath" | "cycle" | "wave" | "dance" | "tempo" | "flash" | "solid";
export interface RgbColor { r: number; g: number; b: number }
export interface KeyboardZone { enabled: boolean; color: RgbColor }
export type KeyboardZones = [KeyboardZone, KeyboardZone, KeyboardZone];
export type KeyboardLightingRequest =
  | { family: "zoneRgb"; brightness: number; effect: KeyboardEffect; allColor: RgbColor; zones: KeyboardZones }
  | { family: "white"; brightness: number };
export interface KeyboardLightingTarget { keyboardType: number; enabled: boolean; settings: KeyboardLightingRequest }
export interface KeyboardSnapshot {
  brightnessLevel: number;
  enabled: boolean;
  rawMode: number;
  effect: KeyboardEffect | null;
  allColor: RgbColor;
  zones: KeyboardZones;
}
export interface KeyboardLightingProfile {
  available: boolean;
  unavailableReason: string | null;
  detectedType: number | null;
  detectedFamily: KeyboardFamily;
  detectionSource: string | null;
  current: KeyboardSnapshot | null;
  baseline: KeyboardSnapshot | null;
  warnings: string[];
}
export interface KeyboardApplyOutcome { ok: boolean; message: string; rollbackError: string | null }
export const readKeyboardLighting = (): Promise<KeyboardLightingProfile> => invoke("read_keyboard_lighting");
export const applyKeyboardLighting = (request: KeyboardLightingRequest): Promise<KeyboardApplyOutcome> =>
  invoke("apply_keyboard_lighting", { request });
export const restoreKeyboardLighting = (): Promise<KeyboardApplyOutcome> => invoke("restore_keyboard_lighting");

export interface TopCoverValues { brightness: number; color: RgbColor }
export interface LightingOptionsValues {
  idleTimer: { enabled: boolean; seconds: number };
  bootOverride: boolean | null;
  logo: KeyboardZone | null;
  lightBar: KeyboardZone | null;
  topCover: TopCoverValues | null;
}
export interface LightingOptionsTarget { keyboardType: number; values: LightingOptionsValues }
export interface LightingOptionsProfile {
  revision: string;
  current: LightingOptionsValues;
  capabilities: { keyboardType: number; peripheralSupportKnown: boolean; bootOverride: boolean; logo: boolean; lightBar: boolean; topCover: boolean };
  canRestore: boolean;
  initial: LightingOptionsValues | null;
}
export const readLightingOptions = (): Promise<LightingOptionsProfile> => invoke("read_lighting_options");
export const applyLightingOptions = (request: { expectedRevision: string; values: LightingOptionsValues }): Promise<void> => invoke("apply_lighting_options", { request });
export const restoreLightingOptions = (expectedRevision: string): Promise<void> => invoke("restore_lighting_options", { expectedRevision });

export type PerkeyPattern =
  | { kind: "static"; colors: RgbColor[] }
  | { kind: "wave" | "snake"; direction: number; color: RgbColor | null }
  | { kind: "breath" | "blink" | "random" | "fade"; color: RgbColor | null }
  | { kind: "scan"; colors: [RgbColor, RgbColor] | null }
  | { kind: "ripple"; reactive: boolean; color: RgbColor | null }
  | { kind: "fn"; color: RgbColor };
export interface PerkeyValues { brightness: number; enabled: boolean; speed: number; pattern: PerkeyPattern }
export interface PerkeyTarget { keyboardType: number; device: PerkeyDevice; values: PerkeyValues }
export interface PerkeyDevice {
  newEffects: boolean; language: number; narrowBorder: boolean; threeColumn: boolean;
  compact: boolean | null; physicalThreeColumn: boolean | null;
}
export interface PerkeyProfile {
  revision: string; current: PerkeyValues; device: PerkeyDevice; presets: PerkeyPattern[];
  canRestore: boolean; reactiveError: string | null; reactiveRunning: boolean;
}
export interface PerkeyRequest { expectedRevision: string; values: PerkeyValues }
export const readPerkeyLighting = (): Promise<PerkeyProfile> => invoke("read_perkey_lighting");
export const applyPerkeyLighting = (request: PerkeyRequest): Promise<void> => invoke("apply_perkey_lighting", { request });
export const restorePerkeyLighting = (): Promise<void> => invoke("restore_perkey_lighting");
