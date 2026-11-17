import type { EnergyPolicy, EnergyPolicyProfile } from "../api/battery";
export const ENERGY_DAYS = ["日", "一", "二", "三", "四", "五", "六"];
export const ENERGY_FLOORS = [40, 50, 60, 70, 80, 90];
export const ENERGY_CEILINGS = [60, 70, 80, 90, 100];
export const energyHour = (value: number) => `${String(value).padStart(2, "0")}:00`;
export function energySummary(policy: EnergyPolicy): string {
  if (!policy.enabled) return "停止节能时段，恢复原充电策略";
  const days = policy.everyDay ? "每天" : ENERGY_DAYS.filter((_, i) => policy.weekdays & (1 << i)).map(d => `周${d}`).join("、");
  const slots = policy.slots.flatMap(slot => slot ? [`${energyHour(slot.start)}–${energyHour(slot.end)}`] : []);
  return `${days} · ${slots.length ? slots.join(" / ") : "无定时时段"} · ${policy.automatic ? "自动" : "自定义"} ${policy.limits.floor}–${policy.limits.ceiling}%`;
}
export function energyError(policy: EnergyPolicy, profile: EnergyPolicyProfile): string | null {
  if (policy.enabled && !policy.everyDay && !policy.weekdays) return "至少选择一天。";
  for (const slot of policy.slots) if (slot && slot.start >= slot.end) return "时段开始须早于结束。";
  const [a, b] = policy.slots;
  if (a && b && a.start < b.end && b.start < a.end) return "两个节能时段不能重叠。";
  if (policy.limits.floor >= policy.limits.ceiling) return "放电下限须低于充电上限。";
  if (policy.automatic && JSON.stringify(policy.limits) !== JSON.stringify(profile.capability.factory)) return "原厂自动阈值已变化，请重新选择自动阈值。";
  return null;
}
