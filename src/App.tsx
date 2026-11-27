/**
 * 应用根（composition root）：主题初始化 + AppShell 外框 + 工作区 +
 * 全局通知 Host。页面状态在根持有（持久化于 Phase 4 设置接入）。
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { getOemStartupNotice, onTrayNavigationRequest, restartApplication, toCommandError } from "./api/client";
import { toast } from "./components/use-toast";
import { AppShell } from "./app/AppShell";
import { ApplicationPreparation, type PreparationFailure } from "./app/ApplicationPreparation";
import { AppWorkspace } from "./app/AppWorkspace";
import { DEFAULT_LOCATION, PROFILES_LOCATION, workspaceLocationKey, type WorkspaceLocation } from "./app/navigation";
import { useScrollActivityMarker } from "./app/useScrollActivityMarker";
import { Toaster } from "./components/Toaster";
import { ApplicationUpdateHost } from "./components/ApplicationUpdateHost";
import { ProfileApplicationHost } from "./components/ProfileApplicationHost";
import { AutomaticDisplayTrialHost } from "./components/AutomaticDisplayTrialHost";
import { OemConflictHost } from "./components/OemConflictHost";
import { useAppPreferences, type PreferencesModel } from "./app/useAppPreferences";
import { useApplicationPreparation } from "./app/useApplicationPreparation";
import { useHotkeyState } from "./app/useHotkeyState";
import { rememberPage, startupPage } from "./lib/startup-page";

const SETTINGS_LOCATION = { ...DEFAULT_LOCATION, page: "settings" } satisfies WorkspaceLocation;
const PREPARATION_MESSAGES = {
  checkingOem: "正在检查原厂控制软件…",
  hardware: "正在初始化硬件控制…",
  services: "正在启动后台服务…",
  ready: "正在读取软件偏好…",
  failed: "",
};

function useInitialLocation(snapshot: PreferencesModel["snapshot"]) {
  const [location, setLocation] = useState<WorkspaceLocation | null>(null);
  useEffect(() => {
    if (!snapshot || location) return;
    try { setLocation(startupPage(snapshot.settings.startupPage)); }
    catch (error) {
      setLocation(DEFAULT_LOCATION);
      toast({ kind: "error", title: "读取最近页面失败", description: String(error) });
    }
  }, [snapshot, location]);
  return [location, setLocation] as const;
}

function preparationFailure(preparation: ReturnType<typeof useApplicationPreparation>, preferences: PreferencesModel,
  onOpenSettings: () => void): PreparationFailure | null {
  if (preparation.error) {
    return { title: "无法读取初始化状态", detail: preparation.error, retryLabel: "重新检查", retry: preparation.recheck };
  }
  if (preparation.snapshot?.stage === "failed") {
    return { title: "初始化失败", detail: preparation.snapshot.detail, retryLabel: "重新启动", retry: restartApplication };
  }
  if (!preferences.snapshot && !preferences.busy && preferences.error) {
    return { title: "无法读取软件偏好", detail: preferences.error, retryLabel: "重新读取",
      retry: preferences.reload, onOpenSettings };
  }
  return null;
}

function useOemStartupNotice(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    void getOemStartupNotice().then(notice => {
      if (disposed || !notice) return;
      toast(notice.kind === "closed"
        ? { kind: "success", title: "已关闭原厂软件", description: notice.executables.join("、"), durationMs: 0 }
        : { kind: "warning", title: "启动时未完全退出原厂软件", durationMs: 0,
            description: notice.closed.length ? `已关闭 ${notice.closed.join("、")}。 ${notice.detail}` : notice.detail });
    }).catch(error => {
      if (!disposed) toast({ kind: "error", title: "无法读取原厂软件启动处理结果", description: toCommandError(error).message });
    });
    return () => { disposed = true; };
  }, [enabled]);
}

export default function App() {
  useScrollActivityMarker();
  const preparation = useApplicationPreparation();
  const nativeReady = preparation.snapshot?.stage === "ready";
  useOemStartupNotice(nativeReady);
  const preferences = useAppPreferences(nativeReady);
  const hotkeys = useHotkeyState(nativeReady);
  const [chosenLocation, setLocation] = useState<WorkspaceLocation | null>(null);
  const [initialLocation, setInitialLocation] = useInitialLocation(preferences.snapshot);
  const location = chosenLocation ?? initialLocation ?? DEFAULT_LOCATION;
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

  if (!initialLocation) {
    const failure = preparationFailure(preparation, preferences, () => {
      setLocation(null);
      setInitialLocation(SETTINGS_LOCATION);
    });
    const stage = preparation.snapshot?.stage;
    return <ApplicationPreparation message={stage ? PREPARATION_MESSAGES[stage] : "正在连接应用…"}
      failure={failure} preferences={preferences} />;
  }

  return (
    <OemConflictHost workspace={workspaceLocationKey(location)}>
      <ApplicationUpdateHost>
        <AppShell location={location} onPageChange={page => navigate({ ...location, page })}
          preferences={preferences} globalInfo={<Toaster coldWarning={hotkeys.warning} dismissColdWarning={hotkeys.dismiss} />}>
          <ProfileApplicationHost />
          <AppWorkspace location={location} onNavigate={navigate}
            requestedProfile={requestedProfile} fanRequest={fanRequest} preferences={preferences}
            navigationRequest={navigationRequest} onNavigated={hotkeys.acknowledgeNavigation} />
        </AppShell>
      </ApplicationUpdateHost>
      <AutomaticDisplayTrialHost />
    </OemConflictHost>
  );
}
