import { HardwareArtwork } from "./HardwareArtwork";

export function GpuPowerVisual({ label, enabled, automatic, showState = true }: {
  label?: string; enabled: boolean | null; automatic: boolean | null; showState?: boolean;
}) {
  return <div className="ccc-gpu-power-visual" data-enabled={enabled === null ? "unknown" : enabled ? "on" : "off"}>
    <HardwareArtwork kind="gpu" />
    <div className="ccc-gpu-power-caption">
      {label && <span className="ccc-control-caption">{label}</span>}
      {showState && <strong className="ccc-gpu-power-state">{enabled === null ? "未知" : enabled ? "开启" : "关闭"}</strong>}
      <span className="ccc-control-caption">自动关断 · {automatic === null ? "未知" : automatic ? "允许" : "关闭"}</span>
    </div>
  </div>;
}
