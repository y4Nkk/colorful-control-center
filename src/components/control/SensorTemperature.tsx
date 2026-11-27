import { memo } from "react";
import type { HardwareSnapshot } from "../../api/client";
import { HARDWARE_HISTORY_GAP_MS, HARDWARE_HISTORY_MS } from "../../app/useHardwareHistory";
import { formatParameter } from "../../lib/format-parameters";
import { timeTrace, valuePct, type ScaleDomain, type TimePoint } from "./plot";
import { TrendChart } from "./TrendChart";

export const HIGH_TEMPERATURE = 95;
export const TEMPERATURE_DOMAIN: ScaleDomain = { min: 20, max: 110 };
const temperatureY = valuePct(TEMPERATURE_DOMAIN);
const TEMPERATURE_TICKS = [TEMPERATURE_DOMAIN.max, HIGH_TEMPERATURE, 60, TEMPERATURE_DOMAIN.min].map(tick => ({
  position: temperatureY(tick), label: `${tick}°`, reference: tick === HIGH_TEMPERATURE || undefined,
}));
const formatTemperature = (value: number) => formatParameter(value, "°C", 1);

export const SensorTemperature = memo(function SensorTemperature({ label, samples, read, colorVar }: {
  label: string;
  samples: readonly HardwareSnapshot[];
  read: (sample: HardwareSnapshot) => number | null;
  colorVar: string;
}) {
  const points: TimePoint[] = samples.map(sample => ({ atMs: sample.sampledAtMs, value: read(sample) }));
  const end = samples[samples.length - 1]?.sampledAtMs ?? 0;
  const start = end - HARDWARE_HISTORY_MS;
  const trace = timeTrace(points, TEMPERATURE_DOMAIN, start, HARDWARE_HISTORY_MS, HARDWARE_HISTORY_GAP_MS,
    value => value >= HIGH_TEMPERATURE);
  const missing = samples.length ? "暂无读数" : "等待采样";
  const minutes = HARDWARE_HISTORY_MS / 60_000;
  return <TrendChart label={`${label} 最近 ${minutes} 分钟温度趋势，${HIGH_TEMPERATURE} °C 高温线`}
    series={{ id: label, color: colorVar, trace }} ticks={TEMPERATURE_TICKS}
    formatValue={formatTemperature} hotAt={HIGH_TEMPERATURE} empty={trace.line ? undefined : missing}
    gapPercent={HARDWARE_HISTORY_GAP_MS / HARDWARE_HISTORY_MS * 100} />;
});
