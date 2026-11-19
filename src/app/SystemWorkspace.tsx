import { useEffect, useState, type ReactNode } from "react";
import { WorkspaceActivity } from "../components/Dialog";
import { SegmentedControl } from "../components/SegmentedControl";
import { ControlWorkspace } from "./ControlWorkspace";
import { CoolingWorkspace } from "./CoolingWorkspace";
import { PerformanceWorkspace } from "./PerformanceWorkspace";
import { ProfilesWorkspace } from "./ProfilesWorkspace";
import { SYSTEM_AREAS, systemAreaTitle, type SystemArea } from "./navigation";
import type { HardwareWorkspaceProps } from "./useControlModel";

type SystemWorkspaceProps = HardwareWorkspaceProps & {
  area: SystemArea;
  onAreaChange: (area: SystemArea) => void;
  onOpenPower: () => void;
  requestedProfile: { name: string } | null;
};

export function SystemWorkspace({ active, model, area, onAreaChange, onOpenPower, requestedProfile }: SystemWorkspaceProps) {
  const [opened, setOpened] = useState<ReadonlySet<SystemArea>>(() => new Set([area]));
  useEffect(() => { setOpened(previous => previous.has(area) ? previous : new Set([...previous, area])); }, [area]);
  const overview = area === "overview", cooling = area === "cooling", performance = area === "performance";
  const profiles = area === "profiles";

  return <div className="ccc-control">
    <SegmentedControl className="ccc-system-areas" label="系统区域" value={area} options={SYSTEM_AREAS}
      onChange={value => onAreaChange(value as SystemArea)} />
    {(overview || opened.has("overview")) && <SystemPane visible={overview} active={active} label={systemAreaTitle("overview")}>
      <ControlWorkspace active={active && overview} model={model} />
    </SystemPane>}
    {(cooling || opened.has("cooling")) && <SystemPane visible={cooling} active={active} label={systemAreaTitle("cooling")}>
      <CoolingWorkspace active={active && cooling} model={model} />
    </SystemPane>}
    {(performance || opened.has("performance")) && <SystemPane visible={performance} active={active} label={systemAreaTitle("performance")}>
      <PerformanceWorkspace active={active && performance} model={model} />
    </SystemPane>}
    {(profiles || opened.has("profiles")) && <SystemPane visible={profiles} active={active} label={systemAreaTitle("profiles")}>
      <ProfilesWorkspace active={active && profiles} requestedProfile={requestedProfile} onOpenRules={onOpenPower} />
    </SystemPane>}
  </div>;
}

function SystemPane({ visible, active, label, children }: {
  visible: boolean; active: boolean; label: string; children: ReactNode;
}) {
  return <WorkspaceActivity.Provider value={active && visible}>
    <div hidden={!visible} role="region" aria-label={label}>{children}</div>
  </WorkspaceActivity.Provider>;
}
