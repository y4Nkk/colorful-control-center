/*
 * 类型化客户端边界。本模块是前端唯一与 Tauri 后端通信的文件（scripts/
 * check-boundaries.mjs 强制）；组件只消费这些类型化封装，不得自行拼命令
 * 字符串或 import @tauri-apps。领域命令族由 api/<域>.ts 拥有，经此处
 * re-export 聚合，消费方统一从 "./api/client" 导入。
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
export * from "./hardware";
export * from "./fans";
export * from "./power";
export * from "./profiles";
