/** 应用设置（自启 + 档案导入导出，对应 src-tauri/src/commands/settings.rs）。 */
import { invoke } from "./client";
import type { OpResult } from "./shared";


export type AutostartLevel = "disabled" | "standard" | "administrator";
export interface AutostartSnapshot { revision: string; level: AutostartLevel; elevated: boolean; canRestore: boolean; legacy: boolean }
export const getAutostart = (): Promise<AutostartSnapshot> => invoke("get_autostart");
export const setAutostart = (expectedRevision: string, level: AutostartLevel, restore: boolean): Promise<AutostartSnapshot> =>
  invoke("set_autostart", { expectedRevision, level, restore });
export const exportProfile = (name: string): Promise<string> => invoke("export_profile", { name });
export const importProfile = (json: string): Promise<OpResult> =>
  invoke("import_profile", { json });

export interface StartupProfile { selection: import("./profiles").BackgroundProfileBinding | null; bindingIssue: string | null; lastRun: string | null }
export const getStartupProfile = (): Promise<StartupProfile> => invoke("get_startup_profile");
export const setStartupProfile = (expected: import("./profiles").ProfileFile | null, expectedMode: import("./profiles").ProfileModeApproval | null): Promise<import("./profiles").BackgroundProfileBinding | null> => invoke("set_startup_profile", { expected, expectedMode });
