import { OemPowerMode } from "../components/control/OemPowerMode";
import { PowerPanel } from "../components/control/PowerPanel";
import { OemPerformanceOptions } from "../components/control/OemPerformanceOptions";
import { GpuTuning } from "../components/performance/GpuTuning";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function PerformanceWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { revisions } = model;
  return <div className="ccc-control ccc-performance">
    <OemPowerMode active={active} revision={revisions.mode} onSettled={() => model.refreshOthers("mode")} />
    <OemPerformanceOptions active={active} revision={revisions.options} onSettled={() => model.refreshOthers("options")} />
    <div className="ccc-tuning-columns">
      <PowerPanel active={active} revision={revisions.power} onSettled={() => model.refreshOthers("power")} />
      <GpuTuning active={active} revision={revisions.gpu} onSettled={() => model.refreshOthers("gpu")} />
    </div>
  </div>;
}
