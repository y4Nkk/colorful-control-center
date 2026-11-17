import { invoke, listenEvent } from "./client";
import type { PowerDeliveryProfile } from "./devices";
import type { GpuTurboProfile } from "./gpu";
import type { OemModeCycleProfile } from "./power";

export type HotkeyScope = "powerMode" | "fan" | "keyboardBrightness" | "keyboardSwitch" | "keyboardZones" | "keyboardColor" | "microphoneMute" | "touchpad" | "fnLock" | "mappings" | "openConsole" | "stateNotifications" | "keyboardBrightnessEvent" | "cameraReports" | "screenBrightness" | "panelHotkey" | "gpuTurbo" | "batteryThresholds" | "powerDeliveryEvent" | "batteryColdWarning" | "batteryLowBrightness" | "batteryLowAction" | "openKeyboard" | "displayRotation" | "batteryAlert" | "systemRecovery" | "systemShutdown" | "lidAction" | "airplaneMode" | "panelEvent";
export type SystemActionKind = "sleep" | "hibernate" | "screenOff" | "shutdown";
export interface LidActionProfile { identity: string | null; current: { plan: string; actions: [SystemActionKind | null, SystemActionKind | null]; modernStandby: boolean } | null; onAc: boolean | null; error: string | null; powerError: string | null }
export interface BatteryLowActionProfile { identity: string | null; current: { plan: string; low: number; critical: number } | null; onAc: boolean | null; percent: number | null; error: string | null; powerError: string | null }
export interface LowBatteryBrightnessSnapshot { rule: { version: [number, number]; customer: number; feature: boolean }; panel: string; plan: string; levels: [number, number]; cache: [number, number] }
export interface LowBatteryBrightnessProfile {
  identity: string | null; current: LowBatteryBrightnessSnapshot | null; onAc: boolean | null; percent: number | null; error: string | null;
  settingTarget: [number, number] | null; endingTarget: [number, number] | null;
  recovery: { record: { phase: "preparing" | "applied" | "restoring"; baseline: LowBatteryBrightnessSnapshot; levelsOwned: [boolean, boolean]; cacheOwned: [boolean, boolean]; version: 2; current: LowBatteryBrightnessSnapshot; pending: { before: LowBatteryBrightnessSnapshot; target: LowBatteryBrightnessSnapshot; levels: [boolean, boolean]; cache: [boolean, boolean] } | null }; active: string; panel: string; levels: [number, number]; cache: [number, number] } | null;
  restoreRevision: string | null; restoreError: string | null;
}
export interface BatteryAlertAudioControl { endpoint: string; volumePart: string; mutePart: string; muted: boolean }
export interface BatteryAlertProfile {
  identity: string | null;
  binding: { brightness: string; customer: number; lowControl: boolean; pcBeep: boolean; codec: string | null } | null;
  brightness: LowBatteryBrightnessProfile | null;
  codec: { current: BatteryAlertAudioControl | null; initial: BatteryAlertAudioControl | null; error: string | null; restoreRevision: string | null; restoreError: string | null };
  playing: boolean; error: string | null;
}
export interface PdEventProfile { identity: string | null; current: PowerDeliveryProfile | null; error: string | null }
export interface BatteryThresholdPlan { id: string; values: [[number, number], [number, number], [number, number]] }
export interface BatteryThresholdSnapshot { active: string; plans: BatteryThresholdPlan[]; cache: [number, number, number] }
export interface BatteryThresholdProfile {
  identity: string | null; current: BatteryThresholdSnapshot | null; error: string | null;
  recovery: { phase: "preparing" | "applied" | "restoring"; baseline: BatteryThresholdSnapshot; currentActive: string;
    current: BatteryThresholdPlan[]; cache: [number, number, number]; owned: [boolean, boolean][][]; cacheOwned: [boolean, boolean, boolean] } | null;
  restoreRevision: string | null; restoreError: string | null;
}
export type PanelHotkeyKind = "blankSaver" | "pointerWake";
export interface PanelHotkeyProfile {
  identity: string | null;
  current: { kind: PanelHotkeyKind; inventory: { monitors: { instance: string; name: string; hardwareIds: string[]; signature: string; fingerprint: string; oled: boolean; status: number; problem: number }[]; unresolved: string[] }; hookFlags: [number, number, number] | null; saver: { path: string; fingerprint: string } | null } | null;
  error: string | null;
  session: { id: number; identity: string; kind: PanelHotkeyKind; preparing: boolean; error: string | null } | null;
  restoreRevision: string | null;
}
export interface ScreenBrightnessProfile {
  identity: string | null;
  current: { reading: { panel: string; current: number; onAc: boolean }; plan: string; levels: [number, number]; targets: number[] } | null;
  error: string | null;
  initial: { plan: string; original: [number, number]; owned: [boolean, boolean]; current: [number, number] | null; error: string | null }[];
  restoreRevision: string | null;
  restoreError: string | null;
}
export interface HotkeySettingsValue { enabled: boolean; actions: HotkeyScope[]; osd: boolean }
export interface BatteryColdWarning { id: number; temperatureC: number; batteryPercent: number }
export interface HotkeyNavigationRequest { id: number; ready: boolean; kind: "keyboard" | "displayRotation" }
export interface HotkeyStatus { running: boolean; error: string | null; lastResult: string | null; coldWarning: BatteryColdWarning | null; navigationRequest: HotkeyNavigationRequest | null }
export interface HotkeyProfile {
  revision: string | null;
  current: HotkeySettingsValue | null;
  recordError: string | null;
  resetRevision: string | null;
  initial: HotkeySettingsValue | null;
  facts: { modeCycle: OemModeCycleProfile | null; capabilities: { scope: HotkeyScope; identity: string | null; reason: string | null }[]; blockedReason: string | null; screenBrightness: ScreenBrightnessProfile | null; panelHotkey: PanelHotkeyProfile | null; gpuTurbo: GpuTurboProfile | null; batteryThresholds: BatteryThresholdProfile | null; pdEvent: PdEventProfile | null; batteryLowBrightness: LowBatteryBrightnessProfile | null; batteryLowAction: BatteryLowActionProfile | null; lidAction: LidActionProfile | null; batteryAlert: BatteryAlertProfile | null };
  authorized: boolean;
  status: HotkeyStatus;
  osdError: string | null;
}
export const readHotkeys = (): Promise<HotkeyProfile> => invoke("read_hotkeys");
export const readHotkeyStatus = (): Promise<HotkeyStatus> => invoke("read_hotkey_status");
export const dismissBatteryColdWarning = (expectedId: number): Promise<void> => invoke("dismiss_battery_cold_warning", { expectedId });
export const acknowledgeHotkeyNavigation = (expectedId: number): Promise<void> => invoke("acknowledge_hotkey_navigation", { expectedId });
export const applyHotkeys = (expectedRevision: string, settings: HotkeySettingsValue): Promise<void> => invoke("apply_hotkeys", { request: { expectedRevision, settings } });
export const restoreHotkeys = (expectedRevision: string): Promise<void> => invoke("restore_hotkeys", { expectedRevision });
export const stopHotkeys = (): Promise<void> => invoke("stop_hotkeys");
export const resetInvalidHotkeys = (expectedRevision: string): Promise<void> => invoke("reset_invalid_hotkeys", { expectedRevision });
export const restoreScreenBrightness = (expectedRevision: string): Promise<void> => invoke("restore_screen_brightness", { expectedRevision });
export const restoreLowBatteryBrightness = (expectedRevision: string): Promise<void> => invoke("restore_low_battery_brightness", { expectedRevision });
export const endPanelHotkey = (expectedRevision: string): Promise<void> => invoke("end_panel_hotkey", { expectedRevision });
export const restoreGpuTurbo = (expectedRevision: string): Promise<void> => invoke("restore_gpu_turbo", { expectedRevision });
export const restoreBatteryThresholds = (expectedRevision: string): Promise<void> => invoke("restore_battery_thresholds", { expectedRevision });
export const restorePdEvent = (expectedRevision: string): Promise<void> => invoke("restore_pd_event", { expectedRevision });
export const onHotkeyStatus = (handler: (status: HotkeyStatus) => void): Promise<() => void> => listenEvent("hotkeys-state", handler);
export const onHardwareControlsChanged = (handler: () => void): Promise<() => void> => listenEvent("hardware-controls-changed", handler);

export const restoreBatteryAlertAudio = (expectedRevision: string): Promise<void> => invoke("restore_battery_alert_audio", { expectedRevision });

