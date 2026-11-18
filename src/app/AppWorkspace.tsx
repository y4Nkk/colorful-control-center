import { useEffect, useState } from "react";
import { PAGES, type PageId } from "./navigation";
import { useControlModel } from "./useControlModel";
import type { PreferencesModel } from "./useAppPreferences";
import type { HotkeyNavigationRequest } from "../api/client";
import { ControlWorkspace } from "./ControlWorkspace";
import { CoolingWorkspace } from "./CoolingWorkspace";
import { PerformanceWorkspace } from "./PerformanceWorkspace";
import { GraphicsWorkspace } from "./GraphicsWorkspace";
import { LightingWorkspace } from "./LightingWorkspace";
import { InputWorkspace } from "./InputWorkspace";
import { BatteryWorkspace } from "./BatteryWorkspace";
import { AutomationWorkspace } from "./AutomationWorkspace";
import { ProfilesWorkspace } from "./ProfilesWorkspace";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { ConfigurationPage } from "./ConfigurationPage";

export function AppWorkspace({ page, requestedProfile, preferences, navigationRequest, onNavigated }: {
  page: PageId; requestedProfile: { name: string } | null; preferences: PreferencesModel;
  navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean>;
}) {
  const [opened, setOpened] = useState<ReadonlySet<PageId>>(() => new Set([page]));
  useEffect(() => { setOpened(current => current.has(page) ? current : new Set([...current, page])); }, [page]);
  const model = useControlModel(page === "dashboard" || page === "cooling");
  const workspaces = {
    dashboard: <ControlWorkspace active={page === "dashboard"} model={model} />,
    cooling: <CoolingWorkspace active={page === "cooling"} model={model} />,
    performance: <PerformanceWorkspace active={page === "performance"} model={model} />,
    graphics: <GraphicsWorkspace active={page === "graphics"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    lighting: <LightingWorkspace active={page === "lighting"} model={model} navigationRequest={navigationRequest} onNavigated={onNavigated} />,
    input: <InputWorkspace active={page === "input"} model={model} />,
    battery: <BatteryWorkspace active={page === "battery"} model={model} />,
    automation: <AutomationWorkspace active={page === "automation"} model={model} />,
    profiles: <ProfilesWorkspace active={page === "profiles"} requestedProfile={requestedProfile} />,
    settings: <SettingsWorkspace active={page === "settings"} model={preferences} />,
  };
  return <>{PAGES.filter(entry => entry.id === page || opened.has(entry.id)).map(entry =>
    <ConfigurationPage key={entry.id} active={page === entry.id}>{workspaces[entry.id]}</ConfigurationPage>)}</>;
}
