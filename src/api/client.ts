/*
 * 类型化客户端聚合边界：本模块拥有唯一 invoke() 与后端事件封装，SDK
 * 访问仅限 src/api/（scripts/check-boundaries.mjs 强制）。组件只消费
 * 类型化 API，不得自行拼命令字符串或 import @tauri-apps。领域 API
 * 由 api/<域>.ts 拥有，经此处 re-export 聚合。
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { toCommandError } from "./shared";

type InvokeArgs = Record<string, unknown>;

export async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return args === undefined ? await tauriInvoke<T>(command) : await tauriInvoke<T>(command, args);
  } catch (error) {
    throw toCommandError(error);
  }
}

/** 后端事件的唯一订阅入口（边界规则与 invoke 相同）。 */
export function listenEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return tauriListen<T>(event, (e) => handler(e.payload));
}

/* 领域模块拥有类型化命令族；本边界聚合其公开面。 */
export * from "./shared";
export * from "./app";
export * from "./app-update";
export * from "./window";
export * from "./hardware";
export * from "./fans";
export * from "./power";
export * from "./supply-mode";
export * from "./profiles";
export * from "./profile-policy";
export * from "./settings";
export * from "./preferences";
export * from "./osd";
export * from "./hotkeys";
export * from "./gpu";
export * from "./lighting";
export * from "./lightbar";
export * from "./keyboard";
export * from "./keymap";
export * from "./devices";
export * from "./usb";
export * from "./lan";
export * from "./battery";
export * from "./multizone";
export * from "./microphone";
export * from "./touchpad";
export * from "./airplane";
export * from "./oem-owner";
