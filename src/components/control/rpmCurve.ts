import type { RpmPoint } from "../../api/client";
import { clampInt } from "./curve";

export const sameRpmCurve = (a: RpmPoint[], b: RpmPoint[]) => a.length === b.length &&
  a.every((p, i) => p.tempC === b[i].tempC && p.rpmPct === b[i].rpmPct);

/** 仅在明确编辑或选择自定义时归入新目标下限；读取时保留原厂停转值。 */
export function normalizeRpmCurve(points: RpmPoint[]): RpmPoint[] {
  let previous = 20;
  return points.map((p, index) => {
    const rpmPct = index === points.length - 1 ? 100 : clampInt(p.rpmPct, previous, 100);
    previous = rpmPct;
    return { tempC: p.tempC, rpmPct };
  });
}

export function rpmLimits(points: RpmPoint[], index: number) {
  const last = index === points.length - 1;
  return {
    temp: { min: index === 0 ? 0 : points[index - 1].tempC + 1, max: last ? 100 : points[index + 1].tempC - 1 },
    rpm: { min: last ? 100 : Math.max(20, points[index - 1]?.rpmPct ?? 20), max: last ? 100 : Math.max(20, points[index + 1].rpmPct) },
  };
}

export function moveRpmPoint(points: RpmPoint[], index: number, tempC: number, rpmPct: number): RpmPoint[] {
  const draft = normalizeRpmCurve(points);
  const limits = rpmLimits(draft, index);
  return draft.map((point, at) => at === index ? {
    tempC: clampInt(tempC, limits.temp.min, limits.temp.max),
    rpmPct: clampInt(rpmPct, limits.rpm.min, limits.rpm.max),
  } : point);
}

/** 固件按温度阈值切换级别；不将其画成线性插值曲线。 */
export function rpmStepPath(points: RpmPoint[], x = (v: number) => v, y = (v: number) => 100 - v): string {
  if (points.length === 0) return "";
  return `M${x(0)},${y(points[0].rpmPct)} H${x(points[0].tempC)} ` + points.slice(1)
    .map((p) => `H${x(p.tempC)} V${y(p.rpmPct)}`).join(" ") + ` H${x(100)}`;
}
