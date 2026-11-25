import { invoke } from "./client";
import type { ProfileFile, BackgroundProfileBinding, ProfileModeApproval } from "./profiles";

export type PowerPolicyTrigger = "ac" | "battery" | "resume";
export interface PowerPolicyRecord { version: 2; enabled: boolean; ac: BackgroundProfileBinding | null; battery: BackgroundProfileBinding | null; resume: BackgroundProfileBinding | null }
export interface PowerPolicySelection { file: ProfileFile; mode: ProfileModeApproval | null }
export interface PowerPolicyRequest { expected: PowerPolicyRecord; enabled: boolean; ac: PowerPolicySelection | null; battery: PowerPolicySelection | null; resume: PowerPolicySelection | null }
export interface PowerPolicyView {
  record: PowerPolicyRecord | null; initial: PowerPolicyRecord | null; paused: boolean; active: boolean;
  source: "ac" | "battery" | "ups" | "unknown" | null; sleeping: boolean; pending: PowerPolicyTrigger | null;
  route: { modernStandby: boolean | null; displayEvents: boolean; warnings: string[] } | null;
  configurationError: string | null; resetRevision: string | null; listenerError: string | null; lastIssue: string | null;
}
export const readPowerProfilePolicy = () => invoke<PowerPolicyView>("read_power_profile_policy");
export const savePowerProfilePolicy = (request: PowerPolicyRequest) => invoke<void>("save_power_profile_policy", { request });
export const setPowerProfilePolicyEnabled = (expected: PowerPolicyRecord, enabled: boolean) =>
  invoke<void>("set_power_profile_policy_enabled", { expected, enabled });
export const pausePowerProfilePolicy = () => invoke<void>("pause_power_profile_policy");
export const restorePowerProfilePolicy = (expected: PowerPolicyRecord) => invoke<void>("restore_power_profile_policy", { expected });
export const reloadPowerProfilePolicy = () => invoke<void>("reload_power_profile_policy");
export const resetPowerProfilePolicy = (expectedRevision: string) => invoke<void>("reset_power_profile_policy", { expectedRevision });
