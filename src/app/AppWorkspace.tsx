import { lazy, Suspense, useEffect, useState } from "react";
import { PAGES, type PageId, type WorkspaceLocation } from "./navigation";
import { useControlModel } from "./useControlModel";
import { useHardwareMonitoring } from "./useHardwareHistory";
import type { PreferencesModel } from "./useAppPreferences";
import type { HotkeyNavigationRequest } from "../api/client";
import { ControlWorkspace } from "./ControlWorkspace";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { WorkspaceActivity } from "../components/Dialog";

const PerformanceWorkspace = lazy(() => import("./PerformanceWorkspace").then(module => ({ default: module.PerformanceWorkspace })));
const DeviceConfigurationWorkspace = lazy(() => import("./DeviceConfigurationWorkspace").then(module => ({ default: module.DeviceConfigurationWorkspace })));
const KeyboardWorkspace = lazy(() => import("./KeyboardWorkspace").then(module => ({ default: module.KeyboardWorkspace })));
const PowerWorkspace = lazy(() => import("./PowerWorkspace").then(module => ({ default: module.PowerWorkspace })));

export function AppWorkspace({ location, onNavigate, requestedProfile, fanRequest, preferences, navigationRequest, onNavigated }: {
  location: WorkspaceLocation; onNavigate: (location: WorkspaceLocation) => void;
  requestedProfile: { name: string } | null; preferences: PreferencesModel;
  fanRequest: number;
  navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean>;
}) {
  const { page, powerArea } = location;
  const [opened, setOpened] = useState<ReadonlySet<PageId>>(() => new Set([page]));
  useEffect(() => { setOpened(current => current.has(page) ? current : new Set([...current, page])); }, [page]);
  const model = useControlModel();
  useHardwareMonitoring(page === "dashboard" || page === "performance");
  const workspaces = {
    dashboard: <ControlWorkspace active={page === "dashboard"} />,
    performance: <PerformanceWorkspace active={page === "performance"} model={model} fanRequest={fanRequest} />,
    graphics: <DeviceConfigurationWorkspace active={page === "graphics"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    keyboard: <KeyboardWorkspace active={page === "keyboard"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    power: <PowerWorkspace active={page === "power"} model={model} area={powerArea} requestedProfile={requestedProfile}
      onAreaChange={area => onNavigate({ ...location, page: "power", powerArea: area })} />,
    settings: <SettingsWorkspace active={page === "settings"} model={preferences} />,
  };
  return <>{PAGES.filter(entry => entry.id === page || opened.has(entry.id)).map(entry => {
    const active = page === entry.id;
    return <WorkspaceActivity.Provider key={entry.id} value={active}>
      <div hidden={!active} className="ccc-page">
        <Suspense fallback={<p className="ccc-control-caption" role="status">正在加载{entry.title}…</p>}>
          {workspaces[entry.id]}
        </Suspense>
      </div>
    </WorkspaceActivity.Provider>;
  })}</>;
}
