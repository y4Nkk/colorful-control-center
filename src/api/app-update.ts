import { Channel } from "@tauri-apps/api/core";
import { invoke } from "./client";

export interface AppUpdate {
  id: number;
  version: string;
  notes: string | null;
}

export type UpdateProgress =
  | { phase: "downloading"; downloadedBytes: number; totalBytes: number | null }
  | { phase: "verifying" | "launching" | "installerOpen" };

export const checkApplicationUpdate = () => invoke<AppUpdate | null>("check_application_update");

export function downloadApplicationUpdate(id: number, onProgress: (progress: UpdateProgress) => void) {
  const progress = new Channel<UpdateProgress>(onProgress);
  return invoke<boolean>("download_application_update", { id, progress });
}

export const cancelApplicationUpdate = (id: number) => invoke<boolean>("cancel_application_update", { id });
