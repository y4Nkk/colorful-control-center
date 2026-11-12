/** 应用设置（自启 + 档案导入导出，对应 src-tauri/src/commands/settings.rs）。 */
import { invoke } from "./client";
import type { OpResult } from "./shared";


export const getAutostart = (): Promise<boolean> => invoke("get_autostart");
export const setAutostart = (enable: boolean): Promise<OpResult> =>
  invoke("set_autostart", { enable });
export const exportProfile = (name: string): Promise<string> => invoke("export_profile", { name });
export const importProfile = (json: string): Promise<OpResult> =>
  invoke("import_profile", { json });
