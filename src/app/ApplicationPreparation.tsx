import { useCallback, useEffect, useState } from "react";
import { exitApplication, isDesktopWindow, openStartupLogs, toCommandError } from "../api/client";
import { BrandLogo } from "../components/BrandLogo";
import { Button } from "../components/Button";
import { Progress } from "../components/Progress";
import type { PreferencesModel } from "./useAppPreferences";
import { WindowControls } from "./WindowControls";

export interface PreparationFailure {
  title: string;
  detail: string;
  retryLabel: string;
  retry: () => void | Promise<unknown>;
  onOpenSettings?: () => void;
}

function PreparationTime({ running }: { running: boolean }) {
  const [started] = useState(() => performance.now());
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!running) return;
    const update = () => setElapsed(Math.floor((performance.now() - started) / 1000));
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [running, started]);
  return <p className="ccc-preparation-time">
    已用时 {Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}
  </p>;
}

export function ApplicationPreparation({ message, failure, preferences }: {
  message: string; failure: PreparationFailure | null; preferences: PreferencesModel;
}) {
  const [actionError, setActionError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const report = useCallback((label: string, error: unknown) => {
    setActionError(`${label}。 ${toCommandError(error).message}`);
  }, []);
  const run = async (label: string, action: () => void | Promise<unknown>) => {
    setPending(true);
    setActionError(null);
    try { await action(); }
    catch (error) { report(`${label}失败`, error); }
    finally { setPending(false); }
  };

  return <div className="ccc-preparation">
    <header className="ccc-topbar ccc-preparation-titlebar" data-tauri-drag-region>
      <span className="ccc-brand ccc-brand-name" data-tauri-drag-region>Colorful Control Center</span>
      <div className="ccc-topbar-spacer" data-tauri-drag-region />
      {isDesktopWindow() && <WindowControls preferences={preferences} preparing onError={report} />}
    </header>
    <main className="ccc-preparation-body" aria-labelledby="ccc-preparation-title">
      <div className="ccc-preparation-content">
        <BrandLogo className="ccc-preparation-logo" />
        <h1 id="ccc-preparation-title">{failure?.title ?? "正在初始化"}</h1>
        <p className="ccc-preparation-message" role={failure ? "alert" : "status"}>{failure?.detail ?? message}</p>
        {!failure && <Progress label="初始化进度" className="ccc-preparation-progress" />}
        <PreparationTime running={!failure} />
        {actionError && <p className="ccc-preparation-error" role="alert">{actionError}</p>}
        <div className="ccc-preparation-actions">
          {failure && <>
            <Button variant="secondary" disabled={pending}
              onClick={() => void run(failure.retryLabel, failure.retry)}>{failure.retryLabel}</Button>
            {failure.onOpenSettings && <Button variant="secondary" disabled={pending}
              onClick={failure.onOpenSettings}>进入设置</Button>}
            <Button variant="secondary" disabled={pending}
              onClick={() => void run("打开日志目录", openStartupLogs)}>打开日志目录</Button>
          </>}
          <Button variant="ghost" disabled={pending} onClick={() => void run("退出应用", exitApplication)}>退出应用</Button>
        </div>
      </div>
    </main>
  </div>;
}
