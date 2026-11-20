/**
 * 应用根（composition root）：主题初始化 + AppShell 外框 + 工作区 +
 * 全局通知 Host。页面状态在根持有（持久化于 Phase 4 设置接入）。
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { onTrayNavigationRequest } from "./api/client";
import { toast } from "./components/use-toast";
import { AppShell } from "./app/AppShell";
import { AppWorkspace } from "./app/AppWorkspace";
import { DEFAULT_LOCATION, PROFILES_LOCATION, workspaceLocationKey, type WorkspaceLocation } from "./app/navigation";
import { useScrollActivityMarker } from "./app/useScrollActivityMarker";
import { Toaster } from "./components/Toaster";
import { ProfileApplicationHost } from "./components/ProfileApplicationHost";
import { AutomaticDisplayTrialHost } from "./components/AutomaticDisplayTrialHost";
import { OemConflictHost } from "./components/OemConflictHost";
import { useAppPreferences } from "./app/useAppPreferences";
import { useHotkeyState } from "./app/useHotkeyState";
import { rememberPage, startupPage } from "./lib/startup-page";

export default function App() {
  useScrollActivityMarker();
  const preferences = useAppPreferences();
  const hotkeys = useHotkeyState();
  const [chosenLocation, setLocation] = useState<WorkspaceLocation | null>(null);
  const [initialLocation, setInitialLocation] = useState<WorkspaceLocation | null>(null);
  const initialized = useRef(false);
  const location: WorkspaceLocation = chosenLocation ?? initialLocation ?? { ...DEFAULT_LOCATION, page: "settings" };
  const currentLocation = useRef(location);
  useLayoutEffect(() => { currentLocation.current = location; }, [location]);
  const navigate = useCallback((next: WorkspaceLocation) => {
    currentLocation.current = next;
    try { rememberPage(next); }
    catch (error) { toast({ kind: "error", title: "保存最近页面失败", description: String(error) }); }
    setLocation(next);
  }, []);
  const navigationRequest = hotkeys.navigationRequest?.ready ? hotkeys.navigationRequest : null;
  useEffect(() => {
    if (navigationRequest === null) return;
    const page = navigationRequest.kind === "keyboard" ? "keyboard" : "graphics";
    navigate({ ...currentLocation.current, page });
  }, [navigationRequest?.id]);
  useEffect(() => {
    if (!preferences.snapshot || initialized.current) return;
    initialized.current = true;
    try { setInitialLocation(startupPage(preferences.snapshot.settings.startupPage)); }
    catch (error) {
      setInitialLocation(DEFAULT_LOCATION);
      toast({ kind: "error", title: "读取最近页面失败", description: String(error) });
    }
  }, [preferences.snapshot]);
  const [requestedProfile, setRequestedProfile] = useState<{ name: string } | null>(null);
  const [fanRequest, setFanRequest] = useState(0);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    onTrayNavigationRequest(request => {
      if (disposed) return;
      if (request.kind === "profile") {
        setRequestedProfile({ name: request.name });
        navigate({ ...currentLocation.current, ...PROFILES_LOCATION });
      } else {
        setFanRequest(previous => previous + 1);
        navigate({ ...currentLocation.current, page: "performance" });
      }
    }).then((off) => { if (disposed) off(); else unlisten = off; })
      .catch((error) => toast({ kind: "error", title: "托盘导航连接失败", description: String(error) }));
    return () => { disposed = true; unlisten?.(); };
  }, [navigate]);

  return (
    <OemConflictHost workspace={workspaceLocationKey(location)}>
      <AppShell location={location} onPageChange={page => navigate({ ...location, page })}
        preferences={preferences}>
        <ProfileApplicationHost />
        <AppWorkspace location={location} onNavigate={navigate}
          requestedProfile={requestedProfile} fanRequest={fanRequest} preferences={preferences}
          navigationRequest={navigationRequest} onNavigated={hotkeys.acknowledgeNavigation} />
      </AppShell>
      <AutomaticDisplayTrialHost />
      <Toaster coldWarning={hotkeys.warning} dismissColdWarning={hotkeys.dismiss} />
    </OemConflictHost>
  );
}
