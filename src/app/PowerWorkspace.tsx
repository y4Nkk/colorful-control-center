import { useEffect, useState } from "react";
import { WorkspaceActivity } from "../components/Dialog";
import { SegmentedControl } from "../components/SegmentedControl";
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
import { ProfilesWorkspace } from "./ProfilesWorkspace";
import { POWER_AREAS, type PowerArea } from "./navigation";

export function PowerWorkspace({ active, model, area, requestedProfile, onAreaChange }: HardwareWorkspaceProps & {
  area: PowerArea; requestedProfile: { name: string } | null; onAreaChange: (area: PowerArea) => void;
}) {
  const [opened, setOpened] = useState<ReadonlySet<PowerArea>>(() => new Set([area]));
  useEffect(() => { setOpened(previous => previous.has(area) ? previous : new Set([...previous, area])); }, [area]);
  return <div className="ccc-control">
    <SegmentedControl className="ccc-power-areas" label="电源区域" value={area} options={POWER_AREAS}
      onChange={value => onAreaChange(value as PowerArea)} />
    {POWER_AREAS.filter(option => option.value === area || opened.has(option.value)).map(({ value, label }) => {
      const visible = value === area;
      return <WorkspaceActivity.Provider key={value} value={active && visible}>
        <div hidden={!visible} role="region" aria-label={label}>
          {value === "settings" ? <PowerSettings active={active && visible} model={model} />
            : <ProfilesWorkspace active={active && visible} requestedProfile={requestedProfile} onOpenRules={() => onAreaChange("settings")} />}
        </div>
      </WorkspaceActivity.Provider>;
    })}
  </div>;
}

function PowerSettings({ active, model }: HardwareWorkspaceProps) {
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
