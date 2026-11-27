import { Button } from "../components/Button";
import { useContext, useEffect, useState } from "react";
import {
  closeWindow, getWindowMaximized, minimizeWindow, onWindowResized, toggleMaximizeWindow, toCommandError,
} from "../api/client";
import { toast } from "../components/use-toast";
import { Icon } from "../components/Icon";
import { Tooltip } from "../components/Tooltip";
import { AdministratorControl } from "./AdministratorControl";
import { AdministratorContext } from "../components/AdministratorHost";
import type { PreferencesModel } from "./useAppPreferences";

function reportError(title: string, error: unknown) {
  toast({ kind: "error", title, description: toCommandError(error).message });
}

function useWindowMaximized(onError: typeof reportError) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let active = true;
    let revision = 0;
    let unlisten: (() => void) | undefined;
    const sync = () => {
      const current = ++revision;
      void getWindowMaximized().then((value) => {
        if (active && current === revision) setMaximized(value);
      }).catch((error: unknown) => {
        if (active && current === revision) onError("无法读取窗口状态", error);
      });
    };
    void onWindowResized(sync).then((stop) => {
      if (!active) { stop(); return; }
      unlisten = stop;
      sync();
    }).catch((error: unknown) => {
      if (active) onError("无法监听窗口变化", error);
    });
    return () => { active = false; unlisten?.(); };
  }, [onError]);
  return maximized;
}

export function WindowControls({ preferences, preparing = false, onError = reportError }: {
  preferences: PreferencesModel; preparing?: boolean; onError?: typeof reportError;
}) {
  const maximized = useWindowMaximized(onError);
  const [windowPending, setWindowPending] = useState(false);
  const administrator = useContext(AdministratorContext);
  const pending = windowPending || administrator?.pending === true;
  const { snapshot, busy, patch } = preferences;
  const { alwaysOnTop, closeBehavior } = snapshot?.settings ?? {};
  const pinLabel = alwaysOnTop ? "取消置顶" : "置顶窗口";
  const maximizeLabel = maximized ? "还原窗口" : "最大化窗口";
  const closeLabel = preparing ? "关闭窗口" : closeBehavior === "exit" ? "关闭窗口并退出应用" : "关闭窗口（隐藏到托盘）";
  const run = async (action: () => Promise<void>, label: string) => {
    setWindowPending(true);
    try { await action(); }
    catch (error) { onError(`${label}失败`, error); }
    finally { setWindowPending(false); }
  };
  return (
    <div className="ccc-window-controls" role="group" aria-label="窗口控制">
      {!preparing && <Tooltip content={pinLabel} side="bottom">
        <Button variant={alwaysOnTop ? "secondary" : "ghost"} size="icon" className="ccc-window-button"
          aria-label="窗口始终置顶" aria-pressed={alwaysOnTop} disabled={pending || busy || alwaysOnTop === undefined}
          onClick={() => void patch({ alwaysOnTop: !alwaysOnTop }, description => {
            toast({ kind: "error", title: `${pinLabel}失败`, description });
          })}>
          <Icon name="pin" size={16} />
        </Button>
      </Tooltip>}
      {!preparing && <AdministratorControl pending={pending} />}
      <Tooltip content="最小化窗口" side="bottom">
        <Button variant="ghost" size="icon" type="button" className="ccc-window-button" aria-label="最小化窗口"
          disabled={pending} onClick={() => void run(minimizeWindow, "最小化窗口")}>
          <Icon name="minimize" size={16} />
        </Button>
      </Tooltip>
      <Tooltip content={maximizeLabel} side="bottom">
        <Button variant="ghost" size="icon" type="button" className="ccc-window-button" aria-label={maximizeLabel}
          disabled={pending} onClick={() => void run(toggleMaximizeWindow, maximizeLabel)}>
          <Icon name={maximized ? "restore" : "maximize"} size={16} />
        </Button>
      </Tooltip>
      <Tooltip content={closeLabel} side="bottom">
        <Button variant="ghost" size="icon" tone="danger" type="button" className="ccc-window-button ccc-window-button-close"
          aria-label="关闭窗口" aria-description={closeLabel}
          disabled={pending || (!preparing && !closeBehavior)} onClick={() => void run(closeWindow, "关闭窗口")}>
          <Icon name="close" size={16} />
        </Button>
      </Tooltip>
    </div>
  );
}
