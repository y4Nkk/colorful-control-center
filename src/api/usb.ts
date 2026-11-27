import { invoke } from "./client";
export const USB_POLICY_LABEL = "USB 选择性暂停";
export type UsbPlanId = "balanced" | "highPerformance" | "powerSaver";
export interface UsbTargets { ac: boolean | null; dc: boolean | null }
export interface UsbPlan extends UsbTargets { id: UsbPlanId }
export interface UsbPolicyTarget { plans: UsbPlan[] }
export interface UsbProfile {
  revision: string; recommendation: boolean | null; supported: boolean; activePlan: UsbPlanId | null;
  activeValues: UsbTargets; plans: UsbPlan[]; missing: UsbPlanId[]; initial: UsbPlan[];
  blockedReason: string | null; restoreBlockedReason: string | null; warnings: string[];
}
export const readUsbPolicy = (): Promise<UsbProfile> => invoke("read_usb_policy");
export const applyUsbPolicy = (expectedRevision: string, values: UsbTargets): Promise<void> => invoke("apply_usb_policy", { request: { expectedRevision, values } });
export const restoreUsbPolicy = (expectedRevision: string): Promise<void> => invoke("restore_usb_policy", { expectedRevision });
