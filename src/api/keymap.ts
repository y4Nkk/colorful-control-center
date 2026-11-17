import { invoke } from "./client";

export type KeymapDevice = "keyboard" | "mouse";
export interface KeymapClick { scanCode: number; makeDelayMs: number; breakDelayMs: number }
export type KeymapStep = { kind: "key"; scanCode: number; pressed: boolean; delayAfterMs: number }
  | { kind: "mouseClick"; button: number; makeDelayMs: number; breakDelayMs: number };
export type KeymapAction = { kind: "keys"; steps: KeymapStep[] }
  | { kind: "text"; text: string; before: KeymapClick[]; after: KeymapClick[] }
  | { kind: "launch"; path: string; eventId: number }
  | { kind: "disable" };
export interface KeymapBinding { trigger: number; action: KeymapAction }
export interface KeymapDeviceSettings { device: KeymapDevice; enabled: boolean; bindings: KeymapBinding[] }
export interface KeymapConfiguration { devices: KeymapDeviceSettings[] }
export interface KeymapTarget { configuration: KeymapConfiguration; programsEnabled: boolean }
export interface KeymapProfile {
  revision: string;
  configuration: KeymapConfiguration | null;
  enabledFlags: [number, number];
  sourceName: string;
  baselineName: string | null;
  canRestore: boolean;
  recoveryRequired: boolean;
  blockedReason: string | null;
  warnings: string[];
  programsRequired: boolean;
  programs: { running: boolean; error: string | null };
}
export const readKeymap = (): Promise<KeymapProfile | null> => invoke("read_keymap");
export const applyKeymap = (expectedRevision: string, configuration: KeymapConfiguration): Promise<void> =>
  invoke("apply_keymap", { request: { expectedRevision, configuration } });
export const restoreKeymap = (expectedRevision: string): Promise<void> => invoke("restore_keymap", { expectedRevision });
export const retryKeymapPrograms = (expectedRevision: string): Promise<void> => invoke("retry_keymap_programs", { expectedRevision });
export interface KeymapFile { version: 1; name: string; configuration: KeymapConfiguration }
export const previewKeymapImport = (json: string): Promise<KeymapFile> => invoke("preview_keymap_import", { json });
export interface KeymapRecording { active: boolean; steps: KeymapStep[]; reason: string | null; elapsedMs: number }
export const startKeymapRecording = (): Promise<KeymapRecording> => invoke("start_keymap_recording");
export const readKeymapRecording = (): Promise<KeymapRecording | null> => invoke("read_keymap_recording");
export const stopKeymapRecording = (): Promise<KeymapRecording | null> => invoke("stop_keymap_recording");
