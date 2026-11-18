import { BatterySettings } from "../components/control/BatterySettings";
import { PlatformPower } from "../components/control/PlatformPower";
import { UsbPowerPolicy } from "../components/control/UsbPowerPolicy";
import { LanPolicy } from "../components/control/LanPolicy";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function BatteryWorkspace({ active, model }: HardwareWorkspaceProps) {
  const { revisions } = model;
  return <div className="ccc-control">
    <BatterySettings active={active} revision={model.deviceRevision} onPolicyChanged={model.refreshSystemPolicy} />
    <PlatformPower active={active} revision={revisions.mode} />
    <div className="ccc-device-pair">
      <UsbPowerPolicy active={active} revision={revisions.mode} />
      <LanPolicy active={active} revision={revisions.mode} />
    </div>
  </div>;
}
