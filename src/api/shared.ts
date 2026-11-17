/** 命令层统一错误形状：后端 panic/拒绝与前端网络失败都归一化到该类型。 */
export class CommandError extends Error {
  readonly code: string;
  constructor(message: string, code = "command-failed") {
    super(message);
    this.name = "CommandError";
    this.code = code;
  }
  override toString(): string { return this.message; }
}

/** 写入类命令的统一结果（Rust commands::OpResult）。 */
export interface OpResult {
  ok: boolean;
  message: string;
}

/** Tauri invoke 失败可能是字符串、Error 或带 message 的对象；统一收敛。 */
export function toCommandError(error: unknown): CommandError {
  if (error instanceof CommandError) return error;
  if (typeof error === "string") return new CommandError(error);
  if (error instanceof Error) return new CommandError(error.message);
  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    const message = typeof record.message === "string" ? record.message : JSON.stringify(error);
    const code = typeof record.code === "string" ? record.code : "command-failed";
    return new CommandError(message, code);
  }
  return new CommandError(String(error));
}
