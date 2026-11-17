import type { LanField, LanValue } from "../../api/client";
export const LAN_FIELDS: Record<LanField, string> = { powerSavingMode: "网卡节能", pnpCapabilities: "唤醒电源策略",
  wakeOnLink: "唤醒联动策略", s5WakeOnLan: "关机唤醒标志", shutdownWake: "关机唤醒支持", wakeOnSlot: "插槽唤醒支持" };
export function lanString(value: LanValue | null | undefined): string | null {
  if (!value || value.kind !== 1 || value.bytes.length < 2 || value.bytes.length % 2 !== 0 || value.bytes[value.bytes.length - 1] !== 0 || value.bytes[value.bytes.length - 2] !== 0) return null;
  try {
    const text = new TextDecoder("utf-16le", { fatal: true, ignoreBOM: true }).decode(new Uint8Array(value.bytes.slice(0, -2)));
    return text.includes("\0") ? null : text;
  } catch { return null; }
}
export function lanValue(value: LanValue | null | undefined): string {
  if (value === undefined) return "本次未修改";
  if (value === null) return "未设置";
  const text = lanString(value);
  if (text !== null) return text || "空文本";
  if (value.kind === 4 && value.bytes.length === 4) return String(new DataView(new Uint8Array(value.bytes).buffer).getUint32(0, true));
  return `原始配置（类型 ${value.kind}，${value.bytes.length} 字节）`;
}
export function lanMeaning(field: LanField, value: LanValue | null | undefined): string {
  if (value == null) return value === null ? "未设置" : "本次未修改";
  const text = lanString(value);
  if (field === "powerSavingMode") return text === "0" ? "关闭" : text === "1" ? "开启" : "未知，保留原始配置";
  if (field === "pnpCapabilities" && value.kind === 4 && lanValue(value) === "256") return "原厂关机唤醒配置";
  if (field === "wakeOnLink" && text === "2") return "原厂唤醒联动配置";
  return "现有驱动配置";
}
