import { Icon } from "../components/Icon";
import { Button } from "../components/Button";
import { Surface } from "../components/Surface";
import { OEM_POWER_MODES, type HardwareSnapshot } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { HardwareReadings } from "../components/control/HardwareReadings";
import { PresetArtwork, presetPalette } from "../components/control/PresetArtwork";
import { useSystemOverview } from "./useSystemOverview";

function ControlStatus({
  live, error, stale, onRetry,
}: {
  live: HardwareSnapshot | null;
  error: string | null;
  stale: boolean;
  onRetry: () => void;
}) {
  const state = stale ? "读数停滞" : live ? null : error ? "监控不可用" : "连接中…";
  const tone = stale ? "ccc-control-caption is-warn" : error && !live ? "ccc-control-error" : "ccc-control-caption";
  const fanOffline = live && !live.dchuAvailable;
  const gpuPaused = live?.nvidiaSuspended;
  if (!state && !error && !fanOffline && !gpuPaused) return null;
  return (
    <div className="ccc-control-status">
      {state && <span className={tone}>{state}</span>}
      {error && <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={onRetry}>重试</Button>}
      {fanOffline && <span className="ccc-control-caption is-warn">风扇通道离线</span>}
      {gpuPaused && <span className="ccc-control-caption">独显监控暂停</span>}
    </div>
  );
}

export function ControlWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { live, samples, error, stale, retry } = model.history;
  const { overview, memoryHistory } = useSystemOverview(active);
  return <div className="ccc-control" role="region" aria-label="概览">
    <CurrentMode mode={stale ? null : live?.oemPowerMode ?? null} reading={!live && !error} active={active} />
    <ControlStatus live={live} error={error} stale={stale} onRetry={retry} />
    <HardwareReadings live={live} samples={samples} overview={overview} memoryHistory={memoryHistory} />
  </div>;
}

function CurrentMode({ mode, reading, active }: { mode: number | null; reading: boolean; active: boolean }) {
  const name = mode == null ? undefined : OEM_POWER_MODES[mode];
  return <Surface as="section" variant="group" className="ccc-panel ccc-current-mode" aria-label="当前模式">
    <h2 className="ccc-panel-title"><Icon name="performance" />当前模式</h2>
    <PresetArtwork mode={mode ?? undefined} active={active} />
    <output className="ccc-preset-label" data-preset-palette={presetPalette(mode)}>{name ?? (reading ? "读取中…" : "未知")}</output>
  </Surface>;
}
