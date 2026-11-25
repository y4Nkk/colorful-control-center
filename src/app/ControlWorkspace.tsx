import { Icon } from "../components/Icon";
import { Button } from "../components/Button";
import { Surface } from "../components/Surface";
import { OEM_POWER_MODES } from "../api/client";
import { HardwareReadings } from "../components/control/HardwareReadings";
import { PresetArtwork, presetPalette } from "../components/control/PresetArtwork";
import { useSystemOverview } from "./useSystemOverview";
import { retryHardwareMonitoring, useHardwareHistory, useHardwareStatus } from "./useHardwareHistory";

function MonitoringStatus({ active }: { active: boolean }) {
  const { connected, error, stale, fanOffline, gpuPaused, mode } = useHardwareStatus(active);
  const state = stale ? "读数停滞" : connected ? null : error ? "监控不可用" : "连接中…";
  const tone = stale ? "ccc-control-caption is-warn" : error && !connected ? "ccc-control-error" : "ccc-control-caption";
  return <>
    <CurrentMode mode={stale ? null : mode} reading={!connected && !error} active={active} />
    {(state || error || fanOffline || gpuPaused) && (
    <div className="ccc-control-status">
      {state && <span className={tone}>{state}</span>}
      {(error || stale) && <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={retryHardwareMonitoring}>重试</Button>}
      {fanOffline && <span className="ccc-control-caption is-warn">风扇通道离线</span>}
      {gpuPaused && <span className="ccc-control-caption">独显监控暂停</span>}
    </div>)}
  </>;
}

export function ControlWorkspace({ active }: { active: boolean }) {
  return <div className="ccc-control" role="region" aria-label="概览">
    <MonitoringStatus active={active} />
    <OverviewReadings active={active} />
  </div>;
}

function OverviewReadings({ active }: { active: boolean }) {
  const { live, samples } = useHardwareHistory(active);
  const { overview, memoryHistory } = useSystemOverview(active);
  return <HardwareReadings live={live} samples={samples} overview={overview} memoryHistory={memoryHistory} />;
}

function CurrentMode({ mode, reading, active }: { mode: number | null; reading: boolean; active: boolean }) {
  const name = mode == null ? undefined : OEM_POWER_MODES[mode];
  return <Surface as="section" variant="group" className="ccc-panel ccc-current-mode" aria-label="当前模式">
    <h2 className="ccc-panel-title"><Icon name="performance" />当前模式</h2>
    <PresetArtwork mode={mode ?? undefined} active={active} />
    <output className="ccc-preset-label" data-preset-palette={presetPalette(mode)}>{name ?? (reading ? "读取中…" : "未知")}</output>
  </Surface>;
}
