/** 应用信息及启动生命周期（对应 src-tauri/src/commands/app.rs）。 */
import { invoke } from "./client";
import type { Appearance } from "./preferences";
export { openUrl as openExternalUrl } from "@tauri-apps/plugin-opener";

export interface BackendInfo {
  name: string;
  version: string;
}

export const getBackendInfo = (): Promise<BackendInfo> => invoke("get_backend_info");

export type AdministratorAccess = "elevated" | "canElevate" | "standard";
export const readAdministratorAccess = (): Promise<AdministratorAccess> => invoke("read_administrator_access");
export const restartApplication = (): Promise<void> => invoke("restart_application");
export const restartAsAdministrator = (): Promise<void> => invoke("restart_as_administrator");

type PreparationStage =
  | { stage: "checkingOem" | "hardware" | "services" | "ready" }
  | { stage: "failed"; detail: string };
export type ApplicationPreparation = PreparationStage & { appearance: Appearance | null };

export const readApplicationPreparation = (): Promise<ApplicationPreparation> => invoke("read_application_preparation");
export const openStartupLogs = (): Promise<void> => invoke("open_startup_logs");
export const exitApplication = (): Promise<void> => invoke("exit_application");
