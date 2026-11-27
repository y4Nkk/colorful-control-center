import { useCallback, useEffect, useRef, useState } from "react";
import { getAppPreferences, onPreferencesError, restartApplication, setAppPreferences, toCommandError,
  type AppPreferences, type PreferencesSnapshot } from "../api/client";
import { applyAppearance, observeSystemTheme } from "../lib/appearance";

export function useAppPreferences(enabled: boolean) {
  const [snapshot, setSnapshot] = useState<PreferencesSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const current = useRef<PreferencesSnapshot | null>(null), running = useRef(false), mounted = useRef(false);
  const accept = useCallback((value: PreferencesSnapshot) => {
    current.current = value;
    applyAppearance(value.settings);
    if (mounted.current) { setSnapshot(value); setError(null); }
  }, []);
  const run = useCallback(async (action: () => Promise<PreferencesSnapshot>, onFailure?: (message: string) => void) => {
    if (running.current) return false;
    running.current = true; setBusy(true); setError(null);
    try { accept(await action()); return true; }
    catch (cause) {
      if (mounted.current) {
        const message = toCommandError(cause).message;
        setError(message);
        onFailure?.(message);
      }
      return false;
    }
    finally { running.current = false; if (mounted.current) setBusy(false); }
  }, [accept]);
  const reload = useCallback(() => run(getAppPreferences), [run]);
  const patch = (update: Partial<AppPreferences>, onFailure?: (message: string) => void) => {
    const saved = current.current?.settings;
    return saved ? run(() => setAppPreferences({ ...saved, ...update }), onFailure) : Promise.resolve(false);
  };
  const restart = async () => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await restartApplication(); }
    catch (cause) {
      running.current = false;
      if (mounted.current) { setError(toCommandError(cause).message); setBusy(false); }
    }
  };
  useEffect(observeSystemTheme, []);
  useEffect(() => {
    if (!enabled) return;
    mounted.current = true;
    void reload();
    let disposed = false, unlisten: (() => void) | undefined;
    void onPreferencesError(message => { if (!disposed) setError(message); })
      .then(off => { if (disposed) off(); else unlisten = off; })
      .catch(cause => { if (!disposed) setError(`软件偏好事件连接失败：${cause}`); });
    return () => { mounted.current = false; disposed = true; unlisten?.(); };
  }, [enabled, reload]);
  return { snapshot, error, busy, patch, reload, restart };
}

export type PreferencesModel = ReturnType<typeof useAppPreferences>;
