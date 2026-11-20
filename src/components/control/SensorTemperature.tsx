import type { HardwareSnapshot } from "../../api/client";
import { HARDWARE_HISTORY_MS } from "../../app/useHardwareHistory";
import { formatParameter } from "../../lib/format-parameters";
import { fmtClock } from "./format";
import { timeTrace, valuePct, type ScaleDomain, type TimePoint } from "./plot";

const HIGH_TEMPERATURE = 95;
const SAMPLE_INTERVAL_MS = 1_000;
/** 固定 20–110 °C 绝对标尺，95 °C 参考线表达距离过热的余量。 */
const TEMPERATURE_DOMAIN: ScaleDomain = { min: 20, max: 110 };
/** 缺测超过一个采样周期即断开折线，不横向拉线。 */
const MAX_GAP_MS = SAMPLE_INTERVAL_MS * 5;

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
  const { segments, markers } = timeTrace(points, TEMPERATURE_DOMAIN, start, HARDWARE_HISTORY_MS, MAX_GAP_MS);
  const line = segments.map(segment => segment.path).join(" ");
  const peak = points.reduce((highest, point) => point.value != null ? Math.max(highest, point.value) : highest, -Infinity);
  const missing = samples.length ? "暂无读数" : "等待采样";
  const limitY = valuePct(TEMPERATURE_DOMAIN)(HIGH_TEMPERATURE);
  return <div className="ccc-sensor-temperature">
    <div className="ccc-sensor-temperature-head">
      <span>温度 · 最近 {HARDWARE_HISTORY_MS / 60_000} 分钟</span>
      <output aria-label={`${label} 当前温度`} className={`ccc-temperature-value ccc-mono${latest != null && latest >= HIGH_TEMPERATURE ? " is-hot" : ""}`}>
        {latest == null ? "—" : formatParameter(latest, "", 1)}{latest != null && <em>°C</em>}
      </output>
    </div>
    <div className="ccc-trend-plot ccc-sensor-temperature-plot" style={{ color: colorVar }} role="img"
      aria-label={`${label} 最近 5 分钟温度折线，${line ? `最高 ${formatParameter(peak, "°C", 1)}` : missing}`}>
      <svg className="ccc-trend-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <path className="ccc-temperature-limit" d={`M0 ${limitY.toFixed(3)}H100`} vectorEffect="non-scaling-stroke" />
        {line && <path className="ccc-trend-line" d={line} stroke="currentColor" vectorEffect="non-scaling-stroke" data-series={label} />}
      </svg>
      {markers.map(marker => <span key={marker.atMs} className="ccc-trend-marker" aria-hidden="true"
        title={`${fmtClock(marker.atMs)} · ${formatParameter(marker.value, "°C", 1)}`}
        style={{ left: `${marker.x}%`, top: `${marker.y}%`, color: marker.value >= HIGH_TEMPERATURE ? "var(--ccc-danger)" : undefined }} />)}
      {!line && <span className="ccc-trend-empty">{missing}</span>}
    </div>
  </div>;
}
