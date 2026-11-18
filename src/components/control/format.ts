/** 监控读数与错误文本格式化（等宽读数统一入口；不美化错误语义）。 */

export function errText(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === "object" && cause !== null) {
    const record = cause as Record<string, unknown>;
    if (typeof record.message === "string") return record.message;
  }
  return String(cause);
}

export function fmtClock(ms: number): string {
  return new Date(ms).toLocaleTimeString("zh-CN", { hour12: false });
}

export function fmtInt(value: number): string {
  return String(Math.round(value));
}

