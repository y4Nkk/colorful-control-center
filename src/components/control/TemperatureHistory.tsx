import type { CSSProperties } from "react";
import type { HardwareSnapshot } from "../../api/client";
import { HARDWARE_HISTORY_MS } from "../../app/useHardwareHistory";
import { formatParameter } from "../../lib/format-parameters";
import { fmtClock } from "./format";

const HIGH_TEMPERATURE = 95;
const SAMPLE_INTERVAL_MS = 1_000;
const minuteWidth = `${60_000 / HARDWARE_HISTORY_MS * 100}%`;

const sensors = [
  { label: "CPU", read: (sample: HardwareSnapshot) => sample.cpuTempC },
  { label: "GPU", read: (sample: HardwareSnapshot) => sample.nvidia?.tempC ?? null },
];
type TemperatureReader = (sample: HardwareSnapshot) => number | null;

function temperatureAt(sample: HardwareSnapshot | undefined, read: TemperatureReader) {
  const value = sample && read(sample);
  return value != null && Number.isFinite(value) ? value : null;
}

function temperatureIntervals(samples: readonly HardwareSnapshot[], read: TemperatureReader) {
  const latest = samples[samples.length - 1];
  if (!latest) return [];
  const windowStart = latest.sampledAtMs - HARDWARE_HISTORY_MS;
  return samples.flatMap((sample, index) => {
    const value = temperatureAt(sample, read);
    const end = sample.sampledAtMs;
    // A sample occupies at most one second and never overlaps the preceding sample.
    const start = Math.max(windowStart, end - SAMPLE_INTERVAL_MS, samples[index - 1]?.sampledAtMs ?? windowStart);
    if (value === null || start >= end) return [];
    return [{
      atMs: end, value,
      left: `${(start - windowStart) / HARDWARE_HISTORY_MS * 100}%`,
      width: `${(end - start) / HARDWARE_HISTORY_MS * 100}%`,
    }];
  });
}

function TemperatureRow({ label, samples, read }: {
  label: string; samples: readonly HardwareSnapshot[]; read: TemperatureReader;
}) {
  const latest = temperatureAt(samples[samples.length - 1], read);
  const intervals = temperatureIntervals(samples, read);
  const peak = intervals.reduce((highest, sample) => Math.max(highest, sample.value), -Infinity);
  const description = intervals.length ? `最高 ${formatParameter(peak, "°C", 1)}` : "暂无读数";
  return <div className="ccc-temperature-row">
    <div className="ccc-temperature-reading">
      <span className="ccc-temperature-sensor">{label}</span>
      <output aria-label={`${label} 当前温度`} className={`ccc-temperature-value ccc-mono${latest != null && latest >= HIGH_TEMPERATURE ? " is-hot" : ""}`}>
        {latest == null ? "—" : formatParameter(latest, "", 1)}{latest != null && <em>°C</em>}
      </output>
    </div>
    <div className="ccc-temperature-track" role="img" aria-label={`${label} 温度时间带，${description}`}
      style={{ "--ccc-temperature-minute": minuteWidth } as CSSProperties}>
      {intervals.map(({ atMs, value, left, width }) => {
        const heat = Math.max(0, Math.min(value / HIGH_TEMPERATURE, 1));
        return <span key={atMs} aria-hidden="true"
          className={`ccc-temperature-sample${value >= HIGH_TEMPERATURE ? " is-hot" : ""}`} data-temperature={value}
          title={`${fmtClock(atMs)} · ${formatParameter(value, "°C", 1)}`}
          style={{ left, width, "--ccc-temperature-heat": `${heat ** 2 * 100}%` } as CSSProperties} />;
      })}
    </div>
  </div>;
}

export function TemperatureHistory({ samples }: { samples: readonly HardwareSnapshot[] }) {
  return <div className="ccc-temperature-history">
    {sensors.map(({ label, read }) => <TemperatureRow key={label} label={label} samples={samples} read={read} />)}
    <div className="ccc-temperature-legend">
      <span><i className="ccc-temperature-swatch is-hot" aria-hidden="true" />≥{HIGH_TEMPERATURE}°C</span>
      <span><i className="ccc-temperature-swatch" aria-hidden="true" />缺测</span>
    </div>
  </div>;
}
