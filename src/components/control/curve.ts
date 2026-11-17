/**
 * 风扇曲线域数学：固件锚点 T1/D1 与终点 100°C/100% 固定，
 * 可调点 T2/D2、T3/D3 单调递增，占空比 ≥ 20%（防失速下限）。
 * 所有写包与草稿都必须经过这里的钳位，前端不发明第二套约束。
 */

export interface CurveDraft {
  t2: number;
  d2Pct: number;
  t3: number;
  d3Pct: number;
}

export interface CurveAnchors {
  t1: number;
  d1Pct: number;
}

export const CURVE_TEMP_MIN = 30;
export const CURVE_TEMP_MAX = 100;
export const CURVE_DUTY_MIN = 20;
export const CURVE_DUTY_MAX = 100;

export function clampInt(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return Math.round(min);
  return Math.min(max, Math.max(min, Math.round(value)));
}

export function clampT2(value: number, anchors: CurveAnchors, t3: number): number {
  const lo = Math.min(Math.round(anchors.t1) + 1, CURVE_TEMP_MAX - 2);
  return clampInt(value, lo, Math.max(lo, t3 - 1));
}

export function clampT3(value: number, t2: number): number {
  const lo = Math.min(t2 + 1, CURVE_TEMP_MAX - 1);
  return clampInt(value, lo, CURVE_TEMP_MAX - 1);
}

export function clampD2(value: number, anchors: CurveAnchors, d3: number): number {
  const lo = Math.max(CURVE_DUTY_MIN, Math.min(Math.floor(anchors.d1Pct) + 1, CURVE_DUTY_MAX - 2));
  return clampInt(value, lo, Math.max(lo, Math.min(CURVE_DUTY_MAX - 2, d3 - 1)));
}

export function clampD3(value: number, d2: number): number {
  const lo = Math.min(Math.max(d2 + 1, CURVE_DUTY_MIN + 1), CURVE_DUTY_MAX - 1);
  return clampInt(value, lo, CURVE_DUTY_MAX - 1);
}

/** 载入时归一：保证任何来路数据都满足单调与范围约束。 */
export function normalizeDraft(anchors: CurveAnchors, raw: CurveDraft): CurveDraft {
  const t2 = clampT2(raw.t2, anchors, raw.t3);
  const t3 = clampT3(raw.t3, t2);
  const d2Pct = clampD2(raw.d2Pct, anchors, raw.d3Pct);
  const d3Pct = clampD3(raw.d3Pct, d2Pct);
  return { t2, d2Pct, t3, d3Pct };
}

export function sameDraft(a: CurveDraft, b: CurveDraft): boolean {
  return a.t2 === b.t2 && a.t3 === b.t3 && a.d2Pct === b.d2Pct && a.d3Pct === b.d3Pct;
}

/** 折线插值：温度在当前曲线上的投影占空比（T1→T2→T3→100°C/100%）。 */
export function dutyAtTemp(temp: number, anchors: CurveAnchors, draft: CurveDraft): number {
  const stops: ReadonlyArray<readonly [number, number]> = [
    [anchors.t1, anchors.d1Pct],
    [draft.t2, draft.d2Pct],
    [draft.t3, draft.d3Pct],
    [CURVE_TEMP_MAX, CURVE_DUTY_MAX],
  ];
  if (temp <= stops[0][0]) return stops[0][1];
  for (let i = 1; i < stops.length; i++) {
    const [t0, d0] = stops[i - 1];
    const [t1, d1] = stops[i];
    if (temp <= t1) {
      const span = Math.max(1e-6, t1 - t0);
      return d0 + ((d1 - d0) * (temp - t0)) / span;
    }
  }
  return CURVE_DUTY_MAX;
}

export function movePoint(anchors: CurveAnchors, draft: CurveDraft, key: "p2" | "p3", temp: number, duty: number): CurveDraft {
  return key === "p2"
    ? { ...draft, t2: clampT2(temp, anchors, draft.t3), d2Pct: clampD2(duty, anchors, draft.d3Pct) }
    : { ...draft, t3: clampT3(temp, draft.t2), d3Pct: clampD3(duty, draft.d2Pct) };
}

export function pointLimits(anchors: CurveAnchors, draft: CurveDraft, point: "p2" | "p3") {
  return point === "p2" ? {
    temp: { min: clampT2(0, anchors, draft.t3), max: clampT2(100, anchors, draft.t3) },
    duty: { min: clampD2(0, anchors, draft.d3Pct), max: clampD2(100, anchors, draft.d3Pct) },
  } : {
    temp: { min: clampT3(0, draft.t2), max: clampT3(100, draft.t2) },
    duty: { min: clampD3(0, draft.d2Pct), max: clampD3(100, draft.d2Pct) },
  };
}
