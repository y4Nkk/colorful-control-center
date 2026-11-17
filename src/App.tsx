/**
 * 应用根（composition root）：主题初始化 + AppShell 外框 + 工作区 +
 * 全局通知 Host。页面状态在根持有（持久化于 Phase 4 设置接入）。
 */
import { useEffect, useRef, useState } from "react";
import { onProfilePreviewRequest } from "./api/client";
import { toast } from "./components/use-toast";
import { AppShell } from "./app/AppShell";
import { AppWorkspace } from "./app/AppWorkspace";
import type { PageId } from "./app/navigation";
import { useScrollActivityMarker } from "./app/useScrollActivityMarker";
import { Toaster } from "./components/Toaster";
import { ProfileApplicationHost } from "./components/ProfileApplicationHost";
import { AutomaticDisplayTrialHost } from "./components/AutomaticDisplayTrialHost";
import { useAppPreferences } from "./app/useAppPreferences";
import { useHotkeyState } from "./app/useHotkeyState";
import { rememberPage, startupPage } from "./lib/startup-page";

export default function App() {
  useScrollActivityMarker();
  const preferences = useAppPreferences();
  const hotkeys = useHotkeyState();
  const [chosenPage, setPage] = useState<PageId | null>(null);
  const [initialPage, setInitialPage] = useState<PageId | null>(null);
  const initialized = useRef(false);
  const page = chosenPage ?? initialPage ?? "settings";
  const navigationRequest = hotkeys.navigationRequest?.ready ? hotkeys.navigationRequest : null;
  useEffect(() => {
    if (navigationRequest === null) return;
    const target = navigationRequest.kind === "keyboard" ? "lighting" : "graphics";
    setPage(target);
    try { rememberPage(target); } catch (error) { toast({ kind: "error", title: "保存最近页面失败", description: String(error) }); }
  }, [navigationRequest?.id]);
  useEffect(() => {
    if (!preferences.snapshot || initialized.current) return;
    initialized.current = true;
    try { setInitialPage(startupPage(preferences.snapshot.settings.startupPage)); }
    catch (error) {
      setInitialPage("dashboard");
      toast({ kind: "error", title: "读取最近页面失败", description: String(error) });
    }
  }, [preferences.snapshot]);
  const navigate = (next: PageId) => {
    try { rememberPage(next); }
    catch (error) { toast({ kind: "error", title: "保存最近页面失败", description: String(error) }); }
    setPage(next);
  };
  const [requestedProfile, setRequestedProfile] = useState<{ name: string } | null>(null);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    onProfilePreviewRequest((name) => {
      if (disposed) return;
      setRequestedProfile({ name });
      setPage("profiles");
      try { rememberPage("profiles"); } catch (error) { toast({ kind: "error", title: "保存最近页面失败", description: String(error) }); }
    }).then((off) => { if (disposed) off(); else unlisten = off; })
      .catch((error) => toast({ kind: "error", title: "托盘档案连接失败", description: String(error) }));
    return () => { disposed = true; unlisten?.(); };
  }, []);

  return (
    <>
      <AppShell page={page} onPageChange={navigate} closeBehavior={preferences.snapshot?.settings.closeBehavior}>
        <ProfileApplicationHost />
        <AppWorkspace page={page} requestedProfile={requestedProfile} preferences={preferences}
          navigationRequest={navigationRequest} onNavigated={hotkeys.acknowledgeNavigation} />
      </AppShell>
      <AutomaticDisplayTrialHost />
      <Toaster coldWarning={hotkeys.warning} dismissColdWarning={hotkeys.dismiss} />
    </>
  );
}
