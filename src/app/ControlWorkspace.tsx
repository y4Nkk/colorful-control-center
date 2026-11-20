import { Icon } from "../components/Icon";
import { Button } from "../components/Button";
import { Surface } from "../components/Surface";
import { DetailsDialog } from "../components/DetailsDialog";
import { OEM_POWER_MODES, type HardwareSnapshot } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { HardwareReadings } from "../components/control/HardwareReadings";
import { PresetChipArtwork } from "../components/control/PresetChipArtwork";
import { fmtClock } from "../components/control/format";
import { useSystemOverview } from "./useSystemOverview";

function ControlStatus({
  live, error, stale, onRetry, system,
}: {
  live: HardwareSnapshot | null;
  error: string | null;
  stale: boolean;
  onRetry: () => void;
  system: ReturnType<typeof useSystemOverview>;
}) {
  const state = stale ? "读数停滞" : live ? `采样 ${fmtClock(live.sampledAtMs)}` : error ? "监控不可用" : "连接中…";
  const tone = stale ? "ccc-control-caption is-warn" : error && !live ? "ccc-control-error" : "ccc-control-caption";
  const samplingIssues = error || (live?.warnings.length ?? 0) > 0;
  const systemIssues = system.error || (system.overview?.warnings.length ?? 0) > 0;
  return (
    <div className="ccc-control-status">
      <span className={tone}>{state}</span>
      {error && <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={onRetry}>重试</Button>}
      {live && !live.dchuAvailable && <span className="ccc-control-caption is-warn">风扇通道离线</span>}
      {live?.nvidiaSuspended && <span className="ccc-control-caption">独显监控暂停</span>}
      {(samplingIssues || systemIssues) && <DetailsDialog title="监控诊断">
        {samplingIssues && <section><h3>硬件采样</h3>
          {error && <p className="ccc-control-error">{error}</p>}
          {live?.warnings.map(warning => <p key={warning}>{warning}</p>)}
          {live?.cpuRemoteRaw != null && <p>CPU 原始温度 {live.cpuRemoteRaw}</p>}
        </section>}
        {systemIssues && <section><h3>系统资料读取</h3>
          {system.error && <p className="ccc-control-error">{system.error}</p>}
          {system.overview?.warnings.map(warning => <p key={warning}>{warning}</p>)}
          <Button variant="ghost" size="compact" disabled={system.reading} aria-busy={system.reading} onClick={system.retry}>
            {system.reading ? "读取中…" : "重新读取"}
          </Button>
        </section>}
      </DetailsDialog>}
    </div>
  );
}

export function ControlWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { live, samples, error, stale, retry } = model.history;
  const system = useSystemOverview(active);
  return <div className="ccc-control" role="region" aria-label="概览">
    <CurrentMode mode={stale ? null : live?.oemPowerMode ?? null} reading={!live && !error} active={active} />
    <ControlStatus live={live} error={error} stale={stale} onRetry={retry} system={system} />
    <HardwareReadings live={live} samples={samples} overview={system.overview} memoryHistory={system.memoryHistory} />
  </div>;
}

function CurrentMode({ mode, reading, active }: { mode: number | null; reading: boolean; active: boolean }) {
  const name = mode == null ? undefined : OEM_POWER_MODES[mode];
  return <Surface as="section" variant="group" className="ccc-panel ccc-current-mode" aria-label="当前模式">
    <h2 className="ccc-panel-title"><Icon name="performance" />当前模式</h2>
    <PresetChipArtwork mode={mode ?? undefined} active={active} />
    <output>{name ?? (reading ? "读取中…" : "未知")}</output>
  </Surface>;
}
