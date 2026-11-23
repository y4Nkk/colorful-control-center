/** 应用外框唯一所有者：自定义标题栏、可折叠导航与保活工作区。 */
import { useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { isDesktopWindow } from "../api/client";
import { BrandLogo } from "../components/BrandLogo";
import { ScrollArea } from "../components/ScrollArea";
import { toast } from "../components/use-toast";
import { pageTitle, workspaceLocationKey, type PageId, type WorkspaceLocation } from "./navigation";
import { SideNav } from "./SideNav";
import type { PreferencesModel } from "./useAppPreferences";
import { WindowControls } from "./WindowControls";

interface AppShellProps {
  location: WorkspaceLocation;
  onPageChange: (page: PageId) => void;
  globalInfo?: ReactNode;
  preferences: PreferencesModel;
  children: ReactNode;
}

const SIDEBAR_STORAGE_KEY = "ccc-sidebar-collapsed";
const compactQuery = window.matchMedia("(max-width: 700px)");
const subscribeCompact = (notify: () => void) => {
  compactQuery.addEventListener("change", notify);
  return () => compactQuery.removeEventListener("change", notify);
};
const getCompact = () => compactQuery.matches;

export function AppShell({ location, onPageChange, globalInfo, preferences, children }: AppShellProps) {
  const { page } = location;
  const scrollKey = workspaceLocationKey(location);
  const viewport = useRef<HTMLDivElement>(null);
  const positions = useRef(new Map<string, number>());
  useLayoutEffect(() => { if (viewport.current) viewport.current.scrollTop = positions.current.get(scrollKey) ?? 0; }, [scrollKey]);
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem(SIDEBAR_STORAGE_KEY) === "true");
  const compact = useSyncExternalStore(subscribeCompact, getCompact);
  const folded = compact || collapsed;
  const toggleSidebar = () => {
    const next = !collapsed;
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, String(next));
      setCollapsed(next);
    } catch (error) {
      toast({ kind: "error", title: "保存侧边栏状态失败", description: String(error) });
    }
  };
  return (
    <div className={`ccc-shell${folded ? " is-collapsed" : ""}`}>
      <SideNav page={page} onPageChange={onPageChange} collapsed={folded} compact={compact}
        onToggle={toggleSidebar} />
      <header className="ccc-topbar" data-tauri-drag-region>
        <h1 className="ccc-brand" data-tauri-drag-region>
          <BrandLogo className="ccc-brand-icon" data-tauri-drag-region />
          <span className="ccc-brand-name" data-tauri-drag-region>Colorful Control Center</span>
        </h1>
        <div className="ccc-global-info">{globalInfo}</div>
        <div className="ccc-topbar-spacer" data-tauri-drag-region />
        {isDesktopWindow() && <WindowControls preferences={preferences} />}
      </header>
      <ScrollArea as="main" surface="workspace" className="ccc-workspace" aria-label={pageTitle(page)} viewportRef={viewport}
        onScroll={event => positions.current.set(scrollKey, event.currentTarget.scrollTop)}>
        {children}
      </ScrollArea>
    </div>
  );
}
