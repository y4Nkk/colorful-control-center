import type { SoftwareCurve, SoftwareCurves, SoftwarePoint } from "../../api/client";
import type { DiffItem } from "./DiffPreview";
import { FAN_LABELS, type FanId } from "../../api/client";

export const SOFTWARE_TEMP_MIN = 20;
export const SOFTWARE_TEMP_MAX = 95;

export const defaultSoftwareCurve = (): SoftwareCurve => ({
  points: [{ tempC: 40, dutyPct: 30 }, { tempC: 60, dutyPct: 45 }, { tempC: 80, dutyPct: 75 }, { tempC: SOFTWARE_TEMP_MAX, dutyPct: 100 }],
  hysteresisC: 3, rampPctPerS: 10,
});
export const sameSoftware = (a: SoftwareCurves | null, b: SoftwareCurves | null) => {
  if (a === null || b === null) return a === b;
  const keys = Object.keys(a).sort();
  return keys.join() === Object.keys(b).sort().join() && keys.every((id) => JSON.stringify(a[id as FanId]) === JSON.stringify(b[id as FanId]));
};
export const softwarePath = (points: SoftwarePoint[]) => `M${SOFTWARE_TEMP_MIN},${100 - points[0].dutyPct} ` + points.map((p) => `L${p.tempC},${100 - p.dutyPct}`).join(" ");
export function softwareLimits(points: SoftwarePoint[], index: number) {
  return { temp: { min: index ? points[index - 1].tempC + 1 : SOFTWARE_TEMP_MIN, max: points[index + 1]?.tempC - 1 || SOFTWARE_TEMP_MAX },
    duty: { min: points[index - 1]?.dutyPct ?? 20, max: points[index + 1]?.dutyPct ?? 100 } };
}
export function moveSoftwarePoint(points: SoftwarePoint[], index: number, temp: number, duty: number): SoftwarePoint[] {
  if (index === points.length - 1) return points;
  const limits = softwareLimits(points, index);
  const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, Math.round(v)));
  return points.map((p, i) => i === index ? { tempC: clamp(temp, limits.temp.min, limits.temp.max), dutyPct: clamp(duty, limits.duty.min, limits.duty.max) } : p);
}
export function addSoftwarePoint(points: SoftwarePoint[]): SoftwarePoint[] {
  if (points.length >= 12) return points;
  let at = -1, largest = 1;
  const all = [{ tempC: SOFTWARE_TEMP_MIN, dutyPct: points[0].dutyPct }, ...points];
  all.slice(1).forEach((p, i) => { if (p.tempC - all[i].tempC > largest) { largest = p.tempC - all[i].tempC; at = i; } });
  if (at < 0) return points;
  const a = all[at], b = all[at + 1];
  return [...points.slice(0, at), { tempC: Math.round((a.tempC + b.tempC) / 2), dutyPct: Math.round((a.dutyPct + b.dutyPct) / 2) }, ...points.slice(at)];
}
export function softwareChanges(before: SoftwareCurves | null, after: SoftwareCurves): DiffItem[] {
  const items: DiffItem[] = [];
  const describe = (c?: SoftwareCurve) => c ? c.points.map((p) => `${p.tempC}°/${p.dutyPct}%`).join(" · ") : "固件控制";
  for (const id of ["cpu", "gpu1"] as const) {
    const a = before?.[id], b = after[id], label = FAN_LABELS[id as FanId];
    if (JSON.stringify(a?.points) !== JSON.stringify(b?.points)) items.push({ label, from: describe(a), to: describe(b) });
    for (const [key, title, unit] of [["hysteresisC", "回差", "°C"], ["rampPctPerS", "斜坡", "%/秒"]] as const) {
      if (b && a?.[key] !== b[key]) items.push({ label: `${label} ${title}`, from: a ? `${a[key]}${unit}` : "—", to: `${b[key]}${unit}` });
    }
  }
  return items;
}
