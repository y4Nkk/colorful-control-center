import { BatterySettings } from "../components/control/BatterySettings";
import { StartupProfilePicker } from "../components/control/StartupProfilePicker";
import { PowerProfilePolicy } from "../components/control/PowerProfilePolicy";
import { SupplyModeSettings } from "../components/control/SupplyModeSettings";
import { PlatformPower } from "../components/control/PlatformPower";
import { UsbPowerPolicy } from "../components/control/UsbPowerPolicy";
import { LanPolicy } from "../components/control/LanPolicy";
import { Icon } from "../components/Icon";
import { Surface } from "../components/Surface";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function PowerWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { deviceRevision, revisions, refreshAll, refreshSystemPolicy } = model;
  return <div className="ccc-control" data-module-layout="power">
    <BatterySettings active={active} revision={deviceRevision} onPolicyChanged={refreshSystemPolicy} />
    <Surface as="section" variant="group" className="ccc-panel ccc-power-profiles" aria-labelledby="power-profile-heading">
      <div className="ccc-panel-head">
        <h2 id="power-profile-heading" className="ccc-panel-title"><Icon name="profiles" />档案应用</h2>
      </div>
      <StartupProfilePicker active={active} />
      <PowerProfilePolicy active={active} revision={deviceRevision} />
    </Surface>
    <SupplyModeSettings active={active} revision={deviceRevision} onSettled={refreshAll} />
    <PlatformPower active={active} revision={revisions.mode} />
    <div className="ccc-module-grid" data-module-layout="adaptive">
      <UsbPowerPolicy active={active} revision={revisions.mode} />
      <LanPolicy active={active} revision={revisions.mode} />
    </div>
  </div>;
}
