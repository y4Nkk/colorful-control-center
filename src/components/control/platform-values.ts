import type { UsbPlanId } from "../../api/usb";

export const USB_PLAN_NAMES: Record<UsbPlanId, string> = { balanced: "平衡", highPerformance: "高性能", powerSaver: "节能" };
export const pdLevelText = (value: number, zeroBased: boolean) => `等级 ${value + Number(zeroBased)}`;
