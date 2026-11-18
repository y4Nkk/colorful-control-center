/**
 * CPU/GPU 分区柱状趋势：一柱对应一次采样，缺失读数留空，不补假值。
 */
import { Fragment } from "react";
import { latestValue, valuePct, type ScaleDomain, type TimePoint } from "./plot";
import { fmtClock, fmtInt } from "./format";

export interface TrendSeries {
  label: string;
  colorVar: string;
  points: TimePoint[];
}

interface TrendChartProps {
  series: TrendSeries[];
  domain: ScaleDomain;
  title: string;
}

function TemperatureBars({ entry, domain, start, span }: {
  entry: TrendSeries;
  domain: ScaleDomain;
  start: number;
  span: number;
}) {
  const y = valuePct(domain);
  const limitY = y(95);
  const hasReadings = entry.points.some((point) => point.value != null && Number.isFinite(point.value));
  // 1Hz 采样只占自身时间槽，不能用柱宽跨过停采时段。
  const width = Math.min(18, 780_000 / span);
  return (
    <div className="ccc-trend-plot" style={{ color: entry.colorVar }} role="img" aria-label={`${entry.label} 温度柱状图，95°C 参考线${hasReadings ? "" : "，暂无读数"}`}>
      <svg className="ccc-trend-columns" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
        <path className="ccc-trend-limit" d={`M0 ${limitY}H1000`} vectorEffect="non-scaling-stroke" />
        {entry.points.map((point, index) => {
          if (point.value == null || !Number.isFinite(point.value)) return null;
          const height = Math.max(0, Math.min(100, 100 - y(point.value)));
          const center = (point.atMs - start) / span * 1000;
          return <rect key={point.atMs}
            className={`ccc-trend-bar${index === entry.points.length - 1 ? " is-latest" : ""}${point.value >= 95 ? " is-hot" : ""}`}
            x={center - width / 2} y={100 - height} width={width} height={height} rx={Math.min(1.8, width / 3)}>
            <title>{entry.label} · {fmtClock(point.atMs)} · {fmtInt(point.value)}°C</title>
          </rect>;
        })}
      </svg>
      {!hasReadings && <span className="ccc-trend-empty">{entry.points.length ? "暂无读数" : "等待采样"}</span>}
      <span className="ccc-trend-limit-label" style={{ top: `${limitY}%` }}>95°C</span>
      <span className="ccc-trend-zero">0°C</span>
    </div>
  );
}

export function TrendChart({ series, domain, title }: TrendChartProps) {
  const points = series[0]?.points ?? [];
  const start = (points[0]?.atMs ?? 0) - 500;
  const span = Math.max(1000, (points[points.length - 1]?.atMs ?? 0) - start + 500);
  return (
    <div className="ccc-trend" role="group" aria-label={title}>
      {series.map((entry) => {
        const latest = latestValue(entry.points);
        return (
          <Fragment key={entry.label}>
            <div className="ccc-trend-chip">
              <span className="ccc-trend-dot" style={{ background: entry.colorVar }} aria-hidden="true" />
              {entry.label}
              <output aria-label={`${entry.label} 当前温度`} className={`ccc-trend-value ccc-mono${latest != null && latest >= 95 ? " is-hot" : ""}`}>
                {latest == null ? "—" : fmtInt(latest)}{latest != null && <em>°C</em>}
              </output>
            </div>
            <TemperatureBars entry={entry} domain={domain} start={start} span={span} />
          </Fragment>
        );
      })}
      <div className="ccc-trend-time ccc-mono" aria-hidden="true">
        <span>{points.length ? fmtClock(points[0].atMs) : ""}</span>
        <span>{points.length > 1 ? fmtClock(points[points.length - 1].atMs) : ""}</span>
      </div>
    </div>
  );
}
