import { invoke, listenEvent } from "./client";

export type ThemePreference = "system" | "light" | "dark";
export type MotionPreference = "system" | "reduce";
export type InterfaceScale = 90 | 100 | 110 | 125;
export type CloseBehavior = "hideToTray" | "exit";
export type StartupPage = "dashboard" | "lastVisited";
export type RuntimeLogLevel = "debug" | "info" | "warn" | "error" | "silent";
export interface AppPreferences {
  theme: ThemePreference;
  interfaceFont: string;
  interfaceScale: InterfaceScale;
  motion: MotionPreference;
  closeBehavior: CloseBehavior;
  alwaysOnTop: boolean;
  startMinimized: boolean;
  /** 已有的保存字段，控制启动和运行期间的自动关闭。 */
  closeOemOnStartup: boolean;
  startupPage: StartupPage;
  globalShortcut: string;
  runtimeLogLevel: RuntimeLogLevel;
  hardwareAcceleration: boolean;
}
export type Appearance = Pick<AppPreferences, "theme" | "interfaceFont" | "interfaceScale" | "motion">;
export interface PreferencesSnapshot {
  settings: AppPreferences;
  desktopError: string | null;
  restartRequired: boolean;
}
export const getAppPreferences = (): Promise<PreferencesSnapshot> => invoke("get_app_preferences");
export const setAppPreferences = (settings: AppPreferences): Promise<PreferencesSnapshot> => invoke("set_app_preferences", { settings });
export const listSystemFonts = (): Promise<string[]> => invoke("list_system_fonts");
export const setShortcutRecording = (enabled: boolean): Promise<void> => invoke("set_shortcut_recording", { enabled });
export const onPreferencesError = (handler: (message: string) => void): Promise<() => void> => listenEvent("preferences-error", handler);
