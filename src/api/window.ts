/** 无边框窗口的类型化 API；组件通过 client.ts 消费。 */
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "./client";

export const isDesktopWindow = (): boolean => isTauri();
export const minimizeWindow = (): Promise<void> => invoke("window_minimize");
export const toggleMaximizeWindow = (): Promise<void> => invoke("window_toggle_maximize");
export const getWindowMaximized = (): Promise<boolean> => invoke("window_is_maximized");
export const closeWindow = (): Promise<void> => invoke("window_close");

export function onWindowResized(handler: () => void): Promise<() => void> {
  return getCurrentWindow().onResized(handler);
}
