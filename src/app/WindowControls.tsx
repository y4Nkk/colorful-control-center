import { Button } from "../components/Button";
import { useEffect, useState } from "react";
import {
  closeWindow, getWindowMaximized, minimizeWindow, onWindowResized, toggleMaximizeWindow, toCommandError,
} from "../api/client";
import { toast } from "../components/use-toast";
import { Icon } from "../components/Icon";
import type { PreferencesModel } from "./useAppPreferences";

function reportError(title: string, error: unknown) {
  toast({ kind: "error", title, description: toCommandError(error).message });
}

function useWindowMaximized() {
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
        if (active && current === revision) reportError("无法读取窗口状态", error);
      });
    };
    void onWindowResized(sync).then((stop) => {
      if (!active) { stop(); return; }
      unlisten = stop;
      sync();
    }).catch((error: unknown) => {
      if (active) reportError("无法监听窗口变化", error);
    });
    return () => { active = false; unlisten?.(); };
  }, []);
  return maximized;
}

export function WindowControls({ preferences }: { preferences: PreferencesModel }) {
  const maximized = useWindowMaximized();
  const [pending, setPending] = useState(false);
  const { snapshot, busy, patch } = preferences;
  const { alwaysOnTop, closeBehavior } = snapshot?.settings ?? {};
  const pinLabel = alwaysOnTop ? "取消置顶" : "置顶窗口";
  const maximizeLabel = maximized ? "还原窗口" : "最大化窗口";
  const run = async (action: () => Promise<void>, label: string) => {
    setPending(true);
    try { await action(); }
    catch (error) { reportError(`${label}失败`, error); }
    finally { setPending(false); }
  };
  return (
    <div className="ccc-window-controls" role="group" aria-label="窗口控制">
      <Button variant={alwaysOnTop ? "secondary" : "ghost"} size="icon" className="ccc-window-button"
        aria-label="窗口始终置顶" aria-pressed={alwaysOnTop} title={pinLabel}
        disabled={pending || busy || alwaysOnTop === undefined}
        onClick={() => void patch({ alwaysOnTop: !alwaysOnTop }, description => {
          toast({ kind: "error", title: `${pinLabel}失败`, description });
        })}>
        <Icon name="pin" size={16} />
      </Button>
      <Button variant="ghost" size="icon" type="button" className="ccc-window-button" aria-label="最小化窗口" title="最小化窗口"
        disabled={pending} onClick={() => void run(minimizeWindow, "最小化窗口")}>
        <Icon name="minimize" size={16} />
      </Button>
      <Button variant="ghost" size="icon" type="button" className="ccc-window-button" aria-label={maximizeLabel} title={maximizeLabel}
        disabled={pending} onClick={() => void run(toggleMaximizeWindow, maximizeLabel)}>
        <Icon name={maximized ? "restore" : "maximize"} size={16} />
      </Button>
      <Button variant="ghost" size="icon" tone="danger" type="button" className="ccc-window-button ccc-window-button-close" aria-label="关闭窗口"
        title={closeBehavior === "exit" ? "关闭窗口并退出应用" : "关闭窗口（隐藏到托盘）"}
        disabled={pending || !closeBehavior} onClick={() => void run(closeWindow, "关闭窗口")}>
        <Icon name="close" size={16} />
      </Button>
    </div>
  );
}
