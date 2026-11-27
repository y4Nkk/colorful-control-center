import { MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS } from "../../app/useSystemOverview";
import { fmtInt } from "./format";
import { timeTrace, type ScaleDomain, type TimePoint } from "./plot";
import { TrendChart } from "./TrendChart";

const MEMORY_DOMAIN: ScaleDomain = { min: 0, max: 100 };
const MEMORY_TICKS = [MEMORY_DOMAIN.max, 75, 50, 25, MEMORY_DOMAIN.min].map(tick => ({
  position: 100 - tick, label: `${tick}%`,
}));
const formatUsage = (value: number) => `${fmtInt(value)}%`;

export function MemoryTrend({ points, color }: { points: readonly TimePoint[]; color: string }) {
  const latest = points[points.length - 1];
  const start = (latest?.atMs ?? 0) - MEMORY_HISTORY_MS;
  const trace = timeTrace(points, MEMORY_DOMAIN, start, MEMORY_HISTORY_MS, MEMORY_MAX_GAP_MS);
  const missing = points.length ? "暂无采样" : "等待采样";
  const minutes = MEMORY_HISTORY_MS / 60_000;

  return <TrendChart label={`最近 ${minutes} 分钟内存使用率面积图`}
    series={{ id: "内存", color, trace }} ticks={MEMORY_TICKS}
    formatValue={formatUsage} empty={trace.line ? undefined : missing} gapPercent={MEMORY_MAX_GAP_MS / MEMORY_HISTORY_MS * 100} />;
}
