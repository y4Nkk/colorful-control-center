import type { GpuTurboLock, GpuTurboOffsets, GpuTurboTarget, GpuModeSkipReason, OemModeGpuReview } from "../../api/client";

export function gpuModeError(review: OemModeGpuReview | null): string | null {
  if (!review) return "完整GPU联动预览未就绪，请重新读取。";
  if (review.status === "unavailable") return review.value;
  const target = review.value;
  if (target.onAc === null) return "实际供电未知，请重读模式GPU目标";
  return target.clocks.driverError ?? target.clocks.candidate.unavailableReason;
}

export const gpuSkipReason = (reason: GpuModeSkipReason) => ({ resume: "唤醒时按原厂规则跳过频率调校", quietPlus: "静音增强不调节GPU频率", notReady: "原厂显卡报告未就绪，跳过频率调校", unsupported: "原厂未声明频率联动能力" })[reason];

export const gpuMhz = (value: number | null) => value === null ? "未知" : `${value / 1000} MHz`;
export const gpuOffsets = (value: GpuTurboOffsets) => `核心 ${gpuMhz(value.coreKhz)} · 显存 ${gpuMhz(value.memKhz)}`;
export function gpuLock(value: GpuTurboLock) {
  if (value.mode === 0) return "未锁定";
  if (value.mode === 2) return `${value.value / 1000} MHz`;
  return `模式 ${value.mode}，原始值 ${value.value}`;
}
export function gpuTarget(value: GpuTurboTarget) {
  const limit = value.lockMhz === null ? "锁频保持" : value.lockMhz === 0 ? "解除锁频" : `锁频 ${value.lockMhz} MHz`;
  const offsets = value.offsets === null ? "偏移保持" : value.offsets.map((v, i) => `GPU ${i + 1} 核心 ${v.coreMhz} / 显存 ${v.memoryMhz} MHz`).join("；");
  return `${limit} · ${offsets}`;
}
export function gpuRestoredOffset(current: number | null | undefined, value: number | null) {
  return value === null ? "保留当前" : `${gpuMhz(current ?? null)} → ${gpuMhz(value)}`;
}
