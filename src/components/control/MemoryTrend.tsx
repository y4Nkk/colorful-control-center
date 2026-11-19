import { useId } from "react";
import { MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS } from "../../app/useSystemOverview";
import { fmtClock, fmtInt } from "./format";
import { timeTrace, type ScaleDomain, type TimePoint } from "./plot";

const MEMORY_DOMAIN: ScaleDomain = { min: 0, max: 100 };

export function MemoryTrend({ points }: { points: readonly TimePoint[] }) {
  const fillId = useId();
  const end = points[points.length - 1]?.atMs;
  const start = (end ?? 0) - MEMORY_HISTORY_MS;
  const { segments, markers } = timeTrace(points, MEMORY_DOMAIN, start, MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS);
  const area = segments.filter(segment => segment.lastX > segment.firstX)
    .map(({ path, firstX, lastX }) => `${path} L${lastX.toFixed(3)} 100 L${firstX.toFixed(3)} 100 Z`).join(" ");
  const line = segments.map(segment => segment.path).join(" ");
  const missing = points.length ? "暂无采样" : "等待采样";

  return <div className="ccc-memory-trend" role="img"
    aria-label={`最近 5 分钟内存使用率面积图${line ? "" : `，${missing}`}`}>
    <div className="ccc-memory-trend-heading">
      <span>使用率 · 最近 5 分钟</span>
      {end != null && <span className="ccc-mono">采样 {fmtClock(end)}</span>}
    </div>
    <div className="ccc-memory-chart">
      <div className="ccc-trend-plot ccc-memory-plot">
        <svg className="ccc-trend-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <defs>
            <linearGradient id={fillId} gradientUnits="userSpaceOnUse" x1="0" x2="0" y1="0" y2="100">
              <stop offset="0" stopColor="var(--ccc-action)" stopOpacity="0.28" />
              <stop offset="1" stopColor="var(--ccc-action)" stopOpacity="0.04" />
            </linearGradient>
          </defs>
          <path className="ccc-memory-midline" d="M0 50H100" vectorEffect="non-scaling-stroke" />
          {area && <path className="ccc-memory-area" d={area} fill={`url(#${fillId})`} />}
          {line && <path className="ccc-trend-line" d={line} stroke="var(--ccc-action)"
            vectorEffect="non-scaling-stroke" data-series="内存" />}
        </svg>
        {markers.map(marker => <span key={marker.atMs} className="ccc-trend-marker"
          title={`${fmtClock(marker.atMs)} · ${fmtInt(marker.value)}%`}
          style={{ left: `${marker.x}%`, top: `${marker.y}%`, color: "var(--ccc-action)" }} aria-hidden="true" />)}
        {!line && <span className="ccc-trend-empty">{missing}</span>}
        {[100, 50, 0].map(tick => <span key={tick} className="ccc-memory-tick ccc-mono" style={{ top: `${100 - tick}%` }}>
          {tick}%
        </span>)}
      </div>
    </div>
  </div>;
}
