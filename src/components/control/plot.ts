/**
 * 温度图表的数据点、数值比例尺与当前读数；缺失值不沿用历史值。
 */

export interface TimePoint {
  atMs: number;
  value: number | null;
}

export interface ScaleDomain {
  min: number;
  max: number;
}

/** 值 → 0..100 纵向百分比（y 自顶向下）。 */
export function valuePct(domain: ScaleDomain): (value: number) => number {
  const span = Math.max(1e-6, domain.max - domain.min);
  return (value) => 100 - ((value - domain.min) / span) * 100;
}

/** 当前采样缺失时返回 null，不沿用历史有效读数。 */
export function latestValue(points: TimePoint[]): number | null {
  const value = points[points.length - 1]?.value;
  return value != null && Number.isFinite(value) ? value : null;
}

