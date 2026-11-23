import type { HardwareSnapshot } from "../../api/client";
import { HARDWARE_HISTORY_MS } from "../../app/useHardwareHistory";
import { formatParameter } from "../../lib/format-parameters";
import { fmtClock } from "./format";
import { timeTrace, valuePct, type ScaleDomain, type TimePoint } from "./plot";

const HIGH_TEMPERATURE = 95;
const TEMPERATURE_DOMAIN: ScaleDomain = { min: 20, max: 110 };
const TEMPERATURE_TICKS = [TEMPERATURE_DOMAIN.max, HIGH_TEMPERATURE, 60, TEMPERATURE_DOMAIN.min];
const temperatureY = valuePct(TEMPERATURE_DOMAIN);
const MAX_GAP_MS = 5_000;

function temperatureAt(sample: HardwareSnapshot | undefined, read: (sample: HardwareSnapshot) => number | null) {
  const value = sample && read(sample);
  return value != null && Number.isFinite(value) ? value : null;
}

export function SensorTemperature({ label, samples, read, colorVar }: {
  label: string;
  samples: readonly HardwareSnapshot[];
  read: (sample: HardwareSnapshot) => number | null;
  colorVar: string;
}) {
  const points: TimePoint[] = samples.map(sample => ({ atMs: sample.sampledAtMs, value: temperatureAt(sample, read) }));
  const latest = temperatureAt(samples[samples.length - 1], read);
  const end = samples[samples.length - 1]?.sampledAtMs ?? 0;
  const start = end - HARDWARE_HISTORY_MS;
  const { line, area, markers } = timeTrace(points, TEMPERATURE_DOMAIN, start, HARDWARE_HISTORY_MS, MAX_GAP_MS,
    value => value >= HIGH_TEMPERATURE);
  const peak = points.reduce((highest, point) => point.value != null ? Math.max(highest, point.value) : highest, -Infinity);
  const missing = samples.length ? "暂无读数" : "等待采样";
  const limitY = temperatureY(HIGH_TEMPERATURE);
  const minutes = HARDWARE_HISTORY_MS / 60_000;
  return <div className="ccc-trend-panel">
    <div className="ccc-trend-head">
      <div>
        <span>当前温度</span>
        <output aria-label={`${label} 当前温度`} className={`ccc-trend-value ccc-mono${latest != null && latest >= HIGH_TEMPERATURE ? " is-hot" : ""}`}>
          {latest == null ? "—" : formatParameter(latest, "", 1)}{latest != null && <em>°C</em>}
        </output>
      </div>
      <div className="ccc-trend-peak">
        <span>{minutes} 分钟最高</span>
        <strong className={`ccc-mono${peak >= HIGH_TEMPERATURE ? " is-hot" : ""}`}>{line ? formatParameter(peak, "°C", 1) : "—"}</strong>
      </div>
    </div>
    <div className="ccc-trend-chart" role="img"
      aria-label={`${label} 最近 ${minutes} 分钟温度趋势，${HIGH_TEMPERATURE} °C 高温线${line ? "" : `，${missing}`}`}>
      <div className="ccc-trend-plot" style={{ color: colorVar }}>
        <svg className="ccc-trend-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <rect className="ccc-temperature-zone" width="100" height={limitY} />
          <path className="ccc-trend-grid" d="M20 0V100 M40 0V100 M60 0V100 M80 0V100" vectorEffect="non-scaling-stroke" />
          {TEMPERATURE_TICKS.map(tick => <path key={tick}
            className={tick === HIGH_TEMPERATURE ? "ccc-temperature-limit" : "ccc-trend-grid"}
            d={`M0 ${temperatureY(tick).toFixed(3)}H100`} vectorEffect="non-scaling-stroke" />)}
          {area && <path className="ccc-trend-area" d={area} />}
          {line && <path className="ccc-trend-line" d={line} vectorEffect="non-scaling-stroke" data-series={label} />}
        </svg>
        {TEMPERATURE_TICKS.map(tick => <span key={tick} className="ccc-trend-tick ccc-mono"
          data-limit={tick === HIGH_TEMPERATURE || undefined} style={{ top: `${temperatureY(tick)}%` }} aria-hidden="true">{tick}°</span>)}
        {markers.map(marker => <span key={marker.atMs} className="ccc-trend-marker" aria-hidden="true"
          data-hot={marker.value >= HIGH_TEMPERATURE || undefined} data-latest={marker.atMs === end || undefined}
          title={`${fmtClock(marker.atMs)} · ${formatParameter(marker.value, "°C", 1)}`}
          style={{ left: `${marker.x}%`, top: `${marker.y}%` }} />)}
        {!line && <span className="ccc-trend-empty">{missing}</span>}
      </div>
      <div className="ccc-trend-time" aria-hidden="true"><span>{minutes} 分钟前</span><span>现在</span></div>
    </div>
  </div>;
}
