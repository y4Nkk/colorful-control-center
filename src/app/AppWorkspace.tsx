import { useEffect, useState } from "react";
import { PAGES, type PageId, type SystemArea, type WorkspaceLocation } from "./navigation";
import { useControlModel } from "./useControlModel";
import type { PreferencesModel } from "./useAppPreferences";
import type { HotkeyNavigationRequest } from "../api/client";
import { SystemWorkspace } from "./SystemWorkspace";
import { DeviceConfigurationWorkspace } from "./DeviceConfigurationWorkspace";
import { KeyboardWorkspace } from "./KeyboardWorkspace";
import { PowerWorkspace } from "./PowerWorkspace";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { WorkspaceActivity } from "../components/Dialog";

export function AppWorkspace({ location, onSystemAreaChange, onOpenPower, requestedProfile, preferences, navigationRequest, onNavigated }: {
  location: WorkspaceLocation; onSystemAreaChange: (area: SystemArea) => void;
  requestedProfile: { name: string } | null; preferences: PreferencesModel;
  onOpenPower: () => void;
  navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean>;
}) {
  const { page, systemArea } = location;
  const [opened, setOpened] = useState<ReadonlySet<PageId>>(() => new Set([page]));
  useEffect(() => { setOpened(current => current.has(page) ? current : new Set([...current, page])); }, [page]);
  const model = useControlModel(page === "dashboard" && systemArea !== "profiles");
  const workspaces = {
    dashboard: <SystemWorkspace active={page === "dashboard"} model={model}
      area={systemArea} onAreaChange={onSystemAreaChange} onOpenPower={onOpenPower} requestedProfile={requestedProfile} />,
    graphics: <DeviceConfigurationWorkspace active={page === "graphics"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    keyboard: <KeyboardWorkspace active={page === "keyboard"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    power: <PowerWorkspace active={page === "power"} model={model} />,
    settings: <SettingsWorkspace active={page === "settings"} model={preferences} />,
  };
  return <>{PAGES.filter(entry => entry.id === page || opened.has(entry.id)).map(entry => {
    const active = page === entry.id;
    return <WorkspaceActivity.Provider key={entry.id} value={active}>
      <div hidden={!active} className="ccc-page">
        {workspaces[entry.id]}
      </div>
    </WorkspaceActivity.Provider>;
  })}</>;
}
