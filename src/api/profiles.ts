/** 性能档案（对应 src-tauri/src/profiles；应用流水线每步独立报告）。 */
import { invoke } from "./client";

export interface ProfileFanPoints {
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}

export interface ProfileSettings {
  fanMode: number | null;
  fanCurves: Record<string, ProfileFanPoints> | null;
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
}

export interface ProfileMeta {
  name: string;
  savedAtMs: number;
}

export interface ApplyStep {
  step: string;
  ok: boolean;
  message: string;
}

export const listProfiles = (): Promise<ProfileMeta[]> => invoke("list_profiles");
export const saveProfile = (name: string, settings: ProfileSettings): Promise<{ ok: boolean; message: string }> =>
  invoke("save_profile", { args: { name, settings } });
export const deleteProfile = (name: string): Promise<{ ok: boolean; message: string }> =>
  invoke("delete_profile", { name });
export const captureProfileSettings = (): Promise<ProfileSettings> => invoke("capture_profile_settings");
export const applyProfile = (name: string): Promise<ApplyStep[]> => invoke("apply_profile", { name });
