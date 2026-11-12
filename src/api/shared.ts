/** 命令层统一错误形状：后端 panic/拒绝与前端网络失败都归一化到该类型。 */
export interface CommandError {
  code: string;
  message: string;
}

/** 写入类命令的统一结果（Rust commands::OpResult）。 */
export interface OpResult {
  ok: boolean;
  message: string;
}

/** Tauri invoke 失败可能是字符串、Error 或带 message 的对象；统一收敛。 */
export function toCommandError(error: unknown): CommandError {
  if (typeof error === "string") return { code: "command-failed", message: error };
  if (error instanceof Error) return { code: "command-failed", message: error.message };
  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    const message = typeof record.message === "string" ? record.message : JSON.stringify(error);
    const code = typeof record.code === "string" ? record.code : "command-failed";
    return { code, message };
  }
  return { code: "command-failed", message: String(error) };
}
