import type { HardwareSnapshot } from "../../api/client";
import { CPU_READING_MAX_AGE_MS, freshCpuReading, HARDWARE_HISTORY_GAP_MS } from "../../app/useHardwareHistory";
import { MetricReadout } from "./MetricReadout";
import { HIGH_TEMPERATURE, TEMPERATURE_DOMAIN, SensorTemperature } from "./SensorTemperature";
import { timeTrace, valuePct, type TimePoint } from "./plot";

const FREQUENCY_WINDOW_MS = 60_000;

function frequencyPoint(atMs: number, mhz: number | null | undefined): TimePoint {
  return { atMs, value: mhz != null && Number.isFinite(mhz) && mhz >= 0 ? mhz : null };
}

const processors = {
  CPU: {
    color: "var(--ccc-chart-blue)",
    fan: "cpu",
    temperature: (sample: HardwareSnapshot) => sample.fan.cpu.tempC,
    utilization: (sample: HardwareSnapshot) => freshCpuReading(sample)?.valuePct ?? null,
    frequency: (sample: HardwareSnapshot) => {
      const reading = freshCpuReading(sample);
      return frequencyPoint(reading?.sampledAtMs ?? sample.sampledAtMs, reading?.clockMhz);
    },
  },
  GPU: {
    color: "var(--ccc-chart-teal)",
    fan: "gpu1",
    temperature: (sample: HardwareSnapshot) => sample.nvidia?.tempC ?? null,
    utilization: (sample: HardwareSnapshot) => sample.nvidia?.utilizationPct ?? null,
    frequency: (sample: HardwareSnapshot) => frequencyPoint(sample.sampledAtMs, sample.nvidia?.graphicsClockMhz),
  },
} as const;

function recentCpuFrequency(live: HardwareSnapshot, samples: readonly HardwareSnapshot[]): number | null {
  if (!freshCpuReading(live)) return null;
  for (let index = samples.length - 1; index >= 0; index--) {
    const reading = freshCpuReading(samples[index]);
    if (!reading || live.sampledAtMs - reading.sampledAtMs > CPU_READING_MAX_AGE_MS) break;
    const frequency = frequencyPoint(reading.sampledAtMs, reading.clockMhz).value;
    if (frequency !== null) return frequency;
  }
  return null;
}

const TEMPERATURE_START_ANGLE = 135;
const TEMPERATURE_SWEEP = 270;
const TEMPERATURE_END_ANGLE = TEMPERATURE_START_ANGLE + TEMPERATURE_SWEEP;
const TEMPERATURE_ARC_RADIUS = 20;
const temperaturePosition = valuePct(TEMPERATURE_DOMAIN);
const temperatureAngle = (value: number) =>
  TEMPERATURE_END_ANGLE - Math.min(100, Math.max(0, temperaturePosition(value))) / 100 * TEMPERATURE_SWEEP;

function arcPoint(angle: number, radius = TEMPERATURE_ARC_RADIUS) {
  const radians = angle * Math.PI / 180;
  return [26 + Math.cos(radians) * radius, 26 + Math.sin(radians) * radius].map(value => value.toFixed(2));
}

function temperatureArc(start: number, end: number) {
  const radius = TEMPERATURE_ARC_RADIUS;
  return `M${arcPoint(start).join(" ")}A${radius} ${radius} 0 ${end - start > 180 ? 1 : 0} 1 ${arcPoint(end).join(" ")}`;
}

const temperatureTrack = temperatureArc(TEMPERATURE_START_ANGLE, TEMPERATURE_END_ANGLE);
const temperatureLimit = temperatureAngle(HIGH_TEMPERATURE);
const hotArc = temperatureArc(temperatureLimit, TEMPERATURE_END_ANGLE);
const hotTick = `M${arcPoint(temperatureLimit, 16).join(" ")}L${arcPoint(temperatureLimit, 24).join(" ")}`;

function TemperatureDial({ value }: { value: number | null }) {
  const reading = value != null && Number.isFinite(value) ? value : null;
  const marker = reading == null ? null : arcPoint(temperatureAngle(reading));
  return <svg viewBox="0 0 52 52" aria-hidden="true" focusable="false" data-plot="temperature">
    <path className="ccc-processor-temp-track" d={temperatureTrack} />
    <path className="ccc-processor-temp-zone" d={hotArc} />
    <path className="ccc-processor-temp-limit" d={hotTick} />
    {marker && <circle className="ccc-processor-temp-point" cx={marker[0]} cy={marker[1]} r="3.8"
      data-hot={reading != null && reading >= HIGH_TEMPERATURE || undefined} />}
  </svg>;
}

function frequencyTrace(samples: readonly HardwareSnapshot[], read: (sample: HardwareSnapshot) => TimePoint) {
  const end = samples[samples.length - 1]?.sampledAtMs ?? 0;
  const start = end - FREQUENCY_WINDOW_MS;
  const points: TimePoint[] = [];
  for (const sample of samples) {
    if (sample.sampledAtMs < start) continue;
    const point = read(sample);
    const previous = points[points.length - 1];
    if (point.atMs >= start && (!previous || point.atMs > previous.atMs)) points.push(point);
  }
  const values = points.flatMap(point => point.value == null ? [] : [point.value]);
  if (values.length === 0) return null;
  const low = Math.min(...values), high = Math.max(...values);
  const padding = Math.max(1, (high - low) * .1);
  return timeTrace(points, { min: low - padding, max: high + padding }, start, FREQUENCY_WINDOW_MS, HARDWARE_HISTORY_GAP_MS);
}

function FrequencySparkline({ samples, read }: {
  samples: readonly HardwareSnapshot[];
  read: (sample: HardwareSnapshot) => TimePoint;
}) {
  const trace = frequencyTrace(samples, read);
  return <svg viewBox="0 0 80 80" aria-hidden="true" focusable="false" data-plot="frequency">
    {trace && <>
      <g transform="translate(4 24) scale(.72 .32)">
        {trace.area && <path className="ccc-processor-frequency-area" d={trace.area} />}
        <path className="ccc-processor-frequency-line" d={trace.line} vectorEffect="non-scaling-stroke" />
      </g>
      {trace.markers.map(point => <circle key={point.atMs} className="ccc-processor-frequency-point"
        cx={4 + point.x * .72} cy={24 + point.y * .32} r="3.2" />)}
    </>}
  </svg>;
}

export function ProcessorReadings({ part, live, samples, retainFrequency = false }: {
  part: keyof typeof processors;
  live: HardwareSnapshot | null;
  samples: readonly HardwareSnapshot[];
  retainFrequency?: boolean;
}) {
  const processor = processors[part];
  const temperature = live ? processor.temperature(live) : null;
  const measuredFrequency = live ? processor.frequency(live).value : null;
  const frequency = measuredFrequency ?? (live && part === "CPU" && retainFrequency ? recentCpuFrequency(live, samples) : null);
  const fan = live?.fan[processor.fan].dutyPct ?? null;
  const strength = fan != null && fan >= 0 && fan <= 100 ? fan : null;
  const usage = live ? processor.utilization(live) : null;

  return <>
    <div className="ccc-overview-metrics">
      <div className="ccc-processor-metrics" style={{ color: processor.color }}>
        <MetricReadout compact label="当前温度" ariaLabel={`${part} 当前温度`} value={temperature} unit="°C"
          alertAt={HIGH_TEMPERATURE} plot={<TemperatureDial value={temperature} />} />
        <MetricReadout compact label={part === "CPU" ? "活跃频率" : "当前频率"} value={frequency} unit="MHz"
          plot={<FrequencySparkline samples={samples} read={processor.frequency} />} />
        <MetricReadout compact label="风扇强度" value={strength} unit="%" maximum={100} ratioShape="arc" colorVar={processor.color} />
      </div>
    </div>
    <div className="ccc-overview-charts">
      <div className="ccc-overview-utilization">
        <MetricReadout ratioShape="arc" label="利用率" ariaLabel={`${part} 利用率`}
          value={usage} maximum={100} unit="%" colorVar={processor.color} />
      </div>
      <SensorTemperature label={part} samples={samples} read={processor.temperature} colorVar={processor.color} />
    </div>
  </>;
}
