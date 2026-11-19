import { Icon } from "../components/Icon";
import { Button } from "../components/Button";
import { Surface } from "../components/Surface";
import { DetailsDialog } from "../components/DetailsDialog";
import { OEM_POWER_MODES, type HardwareSnapshot } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { HardwareReadings } from "../components/control/HardwareReadings";
import { ModeArtwork } from "../components/control/ModeArtwork";
import { TemperatureHistory } from "../components/control/TemperatureHistory";
import { fmtClock } from "../components/control/format";
import { HARDWARE_HISTORY_MS } from "./useHardwareHistory";

function ControlStatus({
  live, error, stale, onRetry,
}: {
  live: HardwareSnapshot | null;
  error: string | null;
  stale: boolean;
  onRetry: () => void;
}) {
  const state = stale ? "读数停滞" : live ? `采样 ${fmtClock(live.sampledAtMs)}` : error ? "监控不可用" : "连接中…";
  const tone = stale ? "ccc-control-caption is-warn" : error && !live ? "ccc-control-error" : "ccc-control-caption";
  return (
    <div className="ccc-control-status">
      <span className={tone}>{state}</span>
      {error && <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={onRetry}>重试</Button>}
      {live && !live.dchuAvailable && <span className="ccc-control-caption is-warn">风扇通道离线</span>}
      {live?.nvidiaSuspended && <span className="ccc-control-caption">独显监控暂停</span>}
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
  return <div className="ccc-control">
    <CurrentMode mode={stale ? null : live?.oemPowerMode ?? null} reading={!live && !error} />
    <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="temperature-heading">
      <div className="ccc-panel-head ccc-overview-head"><h2 id="temperature-heading" className="ccc-panel-title"><Icon name="temperature" />温度趋势</h2>
        <span className="ccc-temperature-range">最近 {HARDWARE_HISTORY_MS / 60_000} 分钟</span>
        <ControlStatus live={live} error={error} stale={stale} onRetry={retry} /></div>
      <TemperatureHistory samples={samples} />
    </Surface>
    <HardwareReadings live={live} active={active} />
  </div>;
}

function CurrentMode({ mode, reading }: { mode: number | null; reading: boolean }) {
  const name = mode == null ? undefined : OEM_POWER_MODES[mode];
  return <Surface as="section" variant="group" className="ccc-panel ccc-current-mode" aria-label="当前模式">
    <div className="ccc-current-mode-copy">
      <h2 className="ccc-panel-title"><Icon name="performance" />当前模式</h2>
      <output>{name ?? (reading ? "读取中…" : "未知")}</output>
    </div>
    <div className="ccc-current-mode-artwork">{mode != null && <ModeArtwork mode={mode} />}</div>
  </Surface>;
}
