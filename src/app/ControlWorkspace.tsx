import { Icon } from "../components/Icon";
import { Button } from "../components/Button";
import { Surface } from "../components/Surface";
import { DetailsDialog } from "../components/DetailsDialog";
import { useMemo } from "react";
import type { HardwareSnapshot } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { HardwareReadings } from "../components/control/HardwareReadings";
import { TrendChart } from "../components/control/TrendChart";
import { type ScaleDomain, type TimePoint } from "../components/control/plot";
import { fmtClock } from "../components/control/format";
const TEMP_DOMAIN: ScaleDomain = { min: 0, max: 110 };
const pick = (samples: HardwareSnapshot[], read: (snapshot: HardwareSnapshot) => number | null): TimePoint[] => samples.map(snapshot => ({ atMs: snapshot.sampledAtMs, value: read(snapshot) }));

function ControlStatus({
  live, error, stale, onRetry,
}: {
  live: HardwareSnapshot | null;
  error: string | null;
  stale: boolean;
  onRetry: () => void;
}) {
  const state = stale ? "读数停滞" : live ? `采样 ${fmtClock(live.sampledAtMs)}` : error ? "监控不可用" : "连接中…";
  return (
    <div className="ccc-control-status">
      <span className={`ccc-chip${stale ? " is-warn" : error && !live ? " is-err" : ""}`}>{state}</span>
      {error && <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={onRetry}>重试</Button>}
      {live && !live.dchuAvailable && <span className="ccc-chip is-warn">风扇通道离线</span>}
      {live?.nvidiaSuspended && <span className="ccc-chip">独显监控暂停</span>}
      {(error || (live?.warnings.length ?? 0) > 0) && <DetailsDialog title="监控诊断">
        {error && <p className="ccc-control-error">{error}</p>}
        {live?.warnings.map((warning) => <p key={warning}>{warning}</p>)}
        {live?.cpuRemoteRaw != null && <p>CPU 原始温度 {live.cpuRemoteRaw}</p>}
      </DetailsDialog>}
    </div>
  );
}

export function ControlWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { live, samples, error, stale, retry } = model.history;
  const tempSeries = useMemo(() => [
    { label: "CPU", colorVar: "var(--ccc-action)", points: pick(samples, s => s.cpuTempC) },
    { label: "GPU", colorVar: "var(--ccc-safe)", points: pick(samples, s => s.nvidia?.tempC ?? null) },
  ], [samples]);
  return <div className="ccc-control">
    <Surface as="section" variant="group" className="ccc-panel" aria-label="温度趋势">
      <div className="ccc-panel-head ccc-overview-head"><h2 className="ccc-panel-title"><Icon name="temperature" />温度</h2>
        <span className="ccc-trend-hint">最近 5 分钟</span>
        <ControlStatus live={live} error={error} stale={stale} onRetry={retry} /></div>
      <TrendChart series={tempSeries} domain={TEMP_DOMAIN} title="CPU 与 GPU 温度趋势（最近 5 分钟）" />
    </Surface>
    <HardwareReadings live={live} active={active} />
  </div>;
}
