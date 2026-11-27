/**
 * 时间图表的数据点和比例尺。缺失读数在曲线上留出缺口。
 */

export interface TimePoint {
  atMs: number;
  value: number | null;
}

export interface ScaleDomain {
  min: number;
  max: number;
}

export interface PlotPoint {
  atMs: number;
  value: number;
  x: number;
  y: number;
}

/** 值 → 0..100 纵向百分比（y 自顶向下）。 */
export function valuePct(domain: ScaleDomain): (value: number) => number {
  const span = Math.max(1e-6, domain.max - domain.min);
  return (value) => 100 - ((value - domain.min) / span) * 100;
}

function hasReading(point: TimePoint | undefined): point is TimePoint & { value: number } {
  return point?.value != null && Number.isFinite(point.value);
}

export function timeTrace(points: readonly TimePoint[], domain: ScaleDomain, start: number, span: number, maxGapMs: number,
  highlight?: (value: number) => boolean,
) {
  const y = valuePct(domain);
  const segments: { path: string; firstX: number; lastX: number }[] = [];
  const positions: PlotPoint[] = [];
  const markers: PlotPoint[] = [];
  let path: string[] = [];
  let firstX = 0;
  points.forEach((point, index) => {
    if (!hasReading(point)) return;
    const previous = points[index - 1], next = points[index + 1];
    const joinsPrevious = hasReading(previous) && point.atMs - previous.atMs <= maxGapMs;
    const joinsNext = hasReading(next) && next.atMs - point.atMs <= maxGapMs;
    const x = (point.atMs - start) / span * 100;
    const height = Math.min(100, Math.max(0, y(point.value)));
    const position = { x, y: height, atMs: point.atMs, value: point.value };
    positions.push(position);
    if (!joinsPrevious) {
      path = [];
      firstX = x;
    }
    path.push(`${joinsPrevious ? "L" : "M"}${x.toFixed(3)} ${height.toFixed(3)}`);
    if (!joinsNext) segments.push({ path: path.join(" "), firstX, lastX: x });
    if ((!joinsPrevious && !joinsNext) || index === points.length - 1 || highlight?.(point.value)) {
      markers.push(position);
    }
  });
  const line = segments.map(segment => segment.path).join(" ");
  const area = segments.filter(segment => segment.lastX > segment.firstX)
    .map(({ path, firstX, lastX }) => `${path} L${lastX.toFixed(3)} 100 L${firstX.toFixed(3)} 100 Z`).join(" ");
  return { line, area, markers, positions };
}
