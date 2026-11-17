import { invoke } from "./client";

export interface KeyboardValues {
  fnLocked: boolean | null;
  winLocked: boolean | null;
  fnWinSwap: boolean | null;
}
export interface KeyboardSettingsProfile {
  revision: string;
  current: KeyboardValues;
  canRestore: boolean;
}
export interface KeyboardSettingsRequest { expectedRevision: string; values: KeyboardValues }
export const readKeyboardSettings = (): Promise<KeyboardSettingsProfile> => invoke("read_keyboard_settings");
export const applyKeyboardSettings = (request: KeyboardSettingsRequest): Promise<void> => invoke("apply_keyboard_settings", { request });
export const restoreKeyboardSettings = (): Promise<void> => invoke("restore_keyboard_settings");
