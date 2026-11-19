/** 配置档案，应用流水线逐步报告结果。 */
import type { RpmPoint, SoftwareCurves, DustSchedule } from "./fans";
import type { RuntimeCpuSettings, OemModeTarget, OemPerformanceValues } from "./power";
import type { LightBarValues } from "./lightbar";
import type { KeyboardValues } from "./keyboard";
import type { KeymapTarget } from "./keymap";
import type { PowerDeliveryTarget, DisplayMode, DisplayProfile, GpuPowerDevice, GraphicsModeProfile, OemModeGpuAction } from "./devices";
import type { GpuModeCycle } from "./gpu";
import type { UsbPolicyTarget } from "./usb";
import type { LanPolicyTarget } from "./lan";
import type { BatteryChargeTarget, BatteryEnergyTarget } from "./battery";
import type { KeyboardLightingTarget, LightingOptionsTarget, PerkeyTarget } from "./lighting";
import type { TouchpadTarget } from "./touchpad";
import type { MicrophoneTarget } from "./microphone";
import type { MultiZoneTarget } from "./multizone";
import { invoke } from "./client";

export interface ProfileFanPoints {
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}

export interface ProfileSettings {
  runtimeCpu: RuntimeCpuSettings | null;
  microphones: MicrophoneTarget | null;
  touchpad: TouchpadTarget | null;
  oemPowerMode: number | null;
  performanceOptions: OemPerformanceValues | null;
  fanMode: number | null;
  fanSoftwareCurves: SoftwareCurves | null;
  fanRpmCurves: Record<string, RpmPoint[]> | null;
  fanCurves: Record<string, ProfileFanPoints> | null;
  pl1W: number | null;
  pl2W: number | null;
  coreOffsetMv: number | null;
  gtOffsetMv: number | null;
  gpu: { coreOffsetMhz: number | null; memOffsetMhz: number | null } | null;
  lightBar: LightBarValues | null;
  dustSchedule: DustSchedule | null;
  keyboard: KeyboardValues | null;
  keyboardLighting: KeyboardLightingTarget | null;
  multiZoneLighting: MultiZoneTarget | null;
  lightingOptions: LightingOptionsTarget | null;
  perkeyLighting: PerkeyTarget | null;
  keymap: KeymapTarget | null;
  powerDelivery: PowerDeliveryTarget | null;
  usbPolicy: UsbPolicyTarget | null;
  batteryCharge: BatteryChargeTarget | null;
  batteryEnergy: BatteryEnergyTarget | null;
  lanPolicy: LanPolicyTarget | null;
  display: { displays: DisplayMode[] } | null;
  gpuPower: { id: string; name: string; values: Pick<GpuPowerDevice, "enabled" | "automaticPowerOff"> } | null;
  graphicsMode: { owner: string; availableMask: number; mode: number } | null;
}
export interface ProfileGraphicsModeReview { before: GraphicsModeProfile; target: number; softwareFanRunning: boolean }
export interface ProfileModeApproval {
  identity: string; target: OemModeTarget; clocks: GpuModeCycle;
  nodeRules: { automaticPowerOff: boolean; action: OemModeGpuAction | null }[];
  allowedAutomaticPowerOff: boolean[] | null;
}
export interface ProfileModeGrant { identity: string; automaticPowerOff: boolean[] | null; driver: string | null; driverDeferred: boolean }
export interface BackgroundProfileBinding { profile: string; digest: string; mode: ProfileModeGrant | null }
export interface ProfileGpuPowerReview { before: GpuPowerDevice; target: NonNullable<ProfileSettings["gpuPower"]>["values"]; softwareFanRunning: boolean }
export interface ProfileDisplayReview { displays: { before: DisplayMode; target: DisplayMode }[] }
export interface ProfileApplication { id: number; revision: number; origin: "manual" | "startup" | "ac" | "battery" | "resume"; targetsApplied: boolean; name: string; steps: ApplyStep[]; pending: DisplayProfile["pending"] }

export type ProfileGroup = "performance" | "display" | "lighting" | "power" | "devices";
export interface CapturedProfile { settings: ProfileSettings; groups: ProfileGroup[] }

export interface ProfileMeta {
  name: string;
  groups: ProfileGroup[];
  revision: string;
}

export interface ManualProfilePreview {
  before: ProfileSettings | null;
  modeTarget: OemModeTarget | null;
  displayReview: ProfileDisplayReview | null;
  gpuPowerReview: ProfileGpuPowerReview | null;
  graphicsModeReview: ProfileGraphicsModeReview | null;
  blocked: string | null;
}

export interface ApplyStep {
  step: string;
  ok: boolean;
  message: string;
}

export const listProfiles = (): Promise<ProfileMeta[]> => invoke("list_profiles");
export const saveProfile = (name: string, settings: ProfileSettings, expectedRevision: string | null): Promise<{ ok: boolean; message: string }> =>
  invoke("save_profile", { args: { name, settings, expectedRevision } });
export const deleteProfile = (name: string, expectedRevision: string): Promise<{ ok: boolean; message: string }> =>
  invoke("delete_profile", { name, expectedRevision });
export const captureProfileSettings = (groups: ProfileGroup[]): Promise<CapturedProfile> => invoke("capture_profile_settings", { groups });
export const previewProfileApplication = (settings: ProfileSettings): Promise<ManualProfilePreview> =>
  invoke("preview_profile_application", { settings });
export const previewProfileAuthorization = (settings: ProfileSettings): Promise<ProfileModeApproval | null> => invoke("preview_profile_authorization", { settings });
export const previewProfileDisplay = (settings: ProfileSettings): Promise<ProfileDisplayReview | null> => invoke("preview_profile_display", { settings });
export const previewProfileGpuPower = (settings: ProfileSettings): Promise<ProfileGpuPowerReview | null> => invoke("preview_profile_gpu_power", { settings });
export const previewProfileGraphicsMode = (settings: ProfileSettings): Promise<ProfileGraphicsModeReview | null> => invoke("preview_profile_graphics_mode", { settings });
export const applyProfile = (expected: ProfileFile, expectedMode: OemModeTarget | null, expectedDisplay: ProfileDisplayReview | null, expectedGpuPower: ProfileGpuPowerReview | null, expectedGraphicsMode: ProfileGraphicsModeReview | null): Promise<ProfileApplication> => invoke("apply_profile", { expected, expectedMode, expectedDisplay, expectedGpuPower, expectedGraphicsMode });
export const readProfileApplication = (): Promise<ProfileApplication | null> => invoke("read_profile_application");
export const keepProfileDisplay = (id: number): Promise<ProfileApplication> => invoke("keep_profile_display", { id });
export const cancelProfileApplication = (id: number): Promise<ProfileApplication> => invoke("cancel_profile_application", { id });

export interface ProfileFile { version: number; name: string; savedAtMs: number; settings: ProfileSettings }
export const loadProfile = (name: string): Promise<ProfileFile> => invoke("load_profile", { name });
export const previewProfileImport = (json: string): Promise<ProfileFile> => invoke("preview_profile_import", { json });
