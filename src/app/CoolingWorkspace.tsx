import { FanPanel } from "../components/control/FanPanel";
import { FanDust } from "../components/control/FanDust";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function CoolingWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { revisions } = model;
  return <div className="ccc-control">
    <FanPanel active={active} revision={revisions.fan} onSettled={() => model.refreshOthers("fan")}
      temps={{ cpu: model.history.live?.cpuTempC ?? null, gpu: model.history.live?.nvidia?.tempC ?? null }} />
    <FanDust active={active} revision={revisions.fan + revisions.mode} onSettled={() => model.setRevisions(previous => ({ ...previous, fan: previous.fan + 1, mode: previous.mode + 1, options: previous.options + 1 }))} />
  </div>;
}
