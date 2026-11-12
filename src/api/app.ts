/** 应用自身信息（对应 src-tauri/src/commands/app.rs）。 */
import { invoke } from "./client";

export interface BackendInfo {
  name: string;
  version: string;
}

export const getBackendInfo = (): Promise<BackendInfo> => invoke("get_backend_info");
