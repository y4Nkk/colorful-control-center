import { MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS } from "../../app/useSystemOverview";
import { fmtClock, fmtInt } from "./format";
import { timeTrace, type ScaleDomain, type TimePoint } from "./plot";

const MEMORY_DOMAIN: ScaleDomain = { min: 0, max: 100 };
const MEMORY_TICKS = [MEMORY_DOMAIN.max, 75, 50, 25, MEMORY_DOMAIN.min];

export function MemoryTrend({ points }: { points: readonly TimePoint[] }) {
  const latest = points[points.length - 1];
  const current = latest?.value ?? null;
  const start = (latest?.atMs ?? 0) - MEMORY_HISTORY_MS;
  const { line, area, markers } = timeTrace(points, MEMORY_DOMAIN, start, MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS);
  const peak = points.reduce<number | null>((highest, { value }) => value == null ? highest : Math.max(highest ?? value, value), null);
  const missing = points.length ? "暂无采样" : "等待采样";
  const minutes = MEMORY_HISTORY_MS / 60_000;

  return <div className="ccc-trend-panel">
    <div className="ccc-trend-head">
      <div>
        <span>当前使用率</span>
        <output className="ccc-trend-value ccc-mono" aria-label="当前内存使用率">
          {current == null ? "—" : <>{fmtInt(current)}<em>%</em></>}
        </output>
      </div>
      <div className="ccc-trend-peak">
        <span>{minutes} 分钟最高</span>
        <strong className="ccc-mono">{peak == null ? "—" : `${fmtInt(peak)}%`}</strong>
      </div>
    </div>
    <div className="ccc-trend-chart ccc-memory-chart" role="img"
      aria-label={`最近 ${minutes} 分钟内存使用率面积图${line ? "" : `，${missing}`}`}>
      <div className="ccc-trend-plot">
        <svg className="ccc-trend-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path className="ccc-trend-grid" d="M20 0V100 M40 0V100 M60 0V100 M80 0V100" vectorEffect="non-scaling-stroke" />
          {MEMORY_TICKS.map(tick => <path key={tick} className="ccc-trend-grid" d={`M0 ${100 - tick}H100`} vectorEffect="non-scaling-stroke" />)}
          {area && <path className="ccc-trend-area" d={area} />}
          {line && <path className="ccc-trend-line" d={line}
            vectorEffect="non-scaling-stroke" data-series="内存" />}
        </svg>
        {markers.map(marker => <span key={marker.atMs} className="ccc-trend-marker"
          title={`${fmtClock(marker.atMs)} · ${fmtInt(marker.value)}%`}
          style={{ left: `${marker.x}%`, top: `${marker.y}%` }} aria-hidden="true" />)}
        {!line && <span className="ccc-trend-empty">{missing}</span>}
        {MEMORY_TICKS.map(tick => <span key={tick} className="ccc-trend-tick ccc-mono" style={{ top: `${100 - tick}%` }} aria-hidden="true">
          {tick}%
        </span>)}
      </div>
      <div className="ccc-trend-time" aria-hidden="true"><span>{minutes} 分钟前</span><span>现在</span></div>
    </div>
  </div>;
}
