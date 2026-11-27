import { memo, useId, useState, type PointerEvent } from "react";
import { fmtClock } from "./format";
import type { PlotPoint, timeTrace } from "./plot";

interface TrendSeries {
  id: string;
  color: string;
  trace: ReturnType<typeof timeTrace>;
}

interface TrendTick {
  position: number;
  label: string;
  reference?: boolean;
}

interface TrendChartProps {
  label: string;
  gapPercent: number;
  series: TrendSeries;
  ticks: readonly TrendTick[];
  formatValue: (value: number) => string;
  empty?: string;
  hotAt?: number;
}

function closestSample(positions: readonly PlotPoint[], position: number, tolerance: number) {
  let sample: PlotPoint | null = null;
  let distance = tolerance;
  for (const point of positions) {
    const offset = Math.abs(point.x - position);
    if (point.x < 0 || point.x > 100 || offset > distance) continue;
    sample = point;
    distance = offset;
  }
  return sample;
}

const TrendPaths = memo(function TrendPaths({ series, fade }: { series: TrendSeries; fade: string }) {
  if (!series.trace.line) return null;
  return <g>
    {series.trace.area && <>
      <linearGradient id={fade} x1="0" y1="0" x2="0" y2="1">
        <stop stopColor="currentColor" stopOpacity=".22" />
        <stop offset="1" stopColor="currentColor" stopOpacity=".02" />
      </linearGradient>
      <path className="ccc-trend-area" d={series.trace.area} fill={`url(#${fade})`} />
    </>}
    <path className="ccc-trend-line" d={series.trace.line} data-series={series.id} vectorEffect="non-scaling-stroke" />
  </g>;
});

const TrendMarkers = memo(function TrendMarkers({ series, hotAt }: Pick<TrendChartProps, "series" | "hotAt">) {
  return series.trace.markers.map(marker => <span key={marker.atMs} className="ccc-trend-marker" aria-hidden="true"
    data-hot={hotAt != null && marker.value >= hotAt || undefined}
    data-latest={marker.x === 100 || undefined}
    style={{ left: `${marker.x}%`, top: `${marker.y}%` }} />);
});

function TrendTooltip({ sample, formatValue, hotAt }: {
  sample: PlotPoint;
  formatValue: TrendChartProps["formatValue"];
  hotAt?: number;
}) {
  return <div className="ccc-trend-hover" aria-hidden="true">
    <span className="ccc-trend-hover-point" data-hot={hotAt != null && sample.value >= hotAt || undefined}
      style={{ left: `${sample.x}%`, top: `${sample.y}%` }} />
    <div className="ccc-trend-tooltip" data-side={sample.x > 55 ? "start" : "end"}>
      <time className="ccc-mono">{fmtClock(sample.atMs)}</time>
      <strong className="ccc-mono">{formatValue(sample.value)}</strong>
    </div>
  </div>;
}

export function TrendChart({ label, gapPercent, series, ticks, formatValue, empty, hotAt }: TrendChartProps) {
  const fade = useId();
  const [position, setPosition] = useState<number | null>(null);
  const sample = position == null ? null : closestSample(series.trace.positions, position, gapPercent / 2);
  const reference = ticks.find(tick => tick.reference);
  const trackPointer = (event: PointerEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    setPosition(bounds.width ? (event.clientX - bounds.left) / bounds.width * 100 : null);
  };
  return <div className="ccc-trend-chart" role="img" aria-label={empty ? `${label}，${empty}` : label}>
    <div className="ccc-trend-plot" style={{ color: series.color }} onPointerMove={trackPointer}
      onPointerLeave={() => setPosition(null)} onPointerCancel={() => setPosition(null)}>
      <svg className="ccc-trend-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        {reference && <rect className="ccc-temperature-zone" width="100" height={reference.position} />}
        {ticks.filter(tick => !tick.reference).map(({ position: y, label: tick }) => <path key={tick}
          className="ccc-trend-grid" d={`M0 ${y.toFixed(3)}H100`} vectorEffect="non-scaling-stroke" />)}
        <TrendPaths series={series} fade={fade} />
        {reference && <path className="ccc-temperature-limit" d={`M0 ${reference.position.toFixed(3)}H100`} vectorEffect="non-scaling-stroke" />}
        {sample && <path className="ccc-trend-cursor" d={`M${sample.x.toFixed(3)} 0V100`} vectorEffect="non-scaling-stroke" />}
      </svg>
      {ticks.map(tick => <span key={tick.label} className="ccc-trend-tick ccc-mono" data-limit={tick.reference}
        style={{ top: `${tick.position}%` }} aria-hidden="true">{tick.label}</span>)}
      <TrendMarkers series={series} hotAt={hotAt} />
      {empty && <span className="ccc-trend-empty">{empty}</span>}
      {sample && <TrendTooltip sample={sample} formatValue={formatValue} hotAt={hotAt} />}
    </div>
  </div>;
}
