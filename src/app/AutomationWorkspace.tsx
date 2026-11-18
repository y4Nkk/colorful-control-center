import { StartupProfilePicker } from "../components/control/StartupProfilePicker";
import { SupplyModeSettings } from "../components/control/SupplyModeSettings";
import { PowerProfilePolicy } from "../components/control/PowerProfilePolicy";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function AutomationWorkspace({ active, model }: HardwareWorkspaceProps) {
  return <div className="ccc-control">
    <StartupProfilePicker active={active} />
    <div className="ccc-device-pair">
      <SupplyModeSettings active={active} revision={model.deviceRevision} onSettled={model.refreshAll} />
      <PowerProfilePolicy active={active} revision={model.deviceRevision} />
    </div>
  </div>;
}
