/** 无边框窗口的类型化 API；组件通过 client.ts 消费。 */
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke, listenEvent } from "./client";

export const isDesktopWindow = (): boolean => isTauri();

export function bundledFontStylesheet(): string {
  const { hostname, protocol } = window.location;
  return isDesktopWindow() && (hostname === "tauri.localhost" || protocol === "tauri:")
    ? convertFileSrc("fonts.css", "cccfonts") : "/fonts/fonts.css";
}

export const minimizeWindow = (): Promise<void> => invoke("window_minimize");
export const toggleMaximizeWindow = (): Promise<void> => invoke("window_toggle_maximize");
export const getWindowMaximized = (): Promise<boolean> => invoke("window_is_maximized");
export const closeWindow = (): Promise<void> => invoke("window_close");

type TrayNavigationRequest = { kind: "cooling" } | { kind: "profile"; name: string };
export const onTrayNavigationRequest = (handler: (request: TrayNavigationRequest) => void): Promise<() => void> =>
  listenEvent<TrayNavigationRequest>("tray-navigation-request", handler);

export function onWindowResized(handler: () => void): Promise<() => void> {
  return getCurrentWindow().onResized(handler);
}
