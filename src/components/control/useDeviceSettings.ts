import { useCallback, useEffect, useRef, useState } from "react";
import { useOemConflict } from "../OemConflictHost";
import { toast } from "../use-toast";

export function useDeviceSettings<T>(read: () => Promise<T>, active: boolean, revision: number | string, pollMs?: number) {
  const conflict = useOemConflict();
  const [profile, setProfile] = useState<T | null>();
  const [readError, setReadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const generation = useRef(0), reading = useRef(false), writing = useRef(false);

  const reload = useCallback(async () => {
    const current = ++generation.current;
    reading.current = true;
    try {
      const next = await read();
      if (current !== generation.current) return false;
      setProfile(next);
      setReadError(null);
      return true;
    } catch (error) {
      if (current === generation.current) {
        setReadError(String(error));
      }
      return false;
    } finally {
      if (current === generation.current) reading.current = false;
    }
  }, [read]);

  useEffect(() => {
    if (!active) return;
    if (!writing.current) void reload();
    if (pollMs === undefined) return;
    const timer = setInterval(() => {
      if (!reading.current && !writing.current) void reload();
    }, pollMs);
    return () => clearInterval(timer);
  }, [active, revision, reload, pollMs]);

  const run = async (action: () => Promise<void>, successTitle?: string) => {
    if (!active || writing.current) return;
    writing.current = true;
    // A read started before this action cannot update the displayed state during submission.
    ++generation.current;
    setBusy(true);
    try {
      await action();
      const refreshed = await reload();
      setActionError(null);
      if (refreshed && successTitle) toast({ kind: "success", title: successTitle });
    } catch (error) {
      const message = String(error);
      setActionError(message);
      await reload();
      conflict.review([message], reread);
    } finally {
      writing.current = false;
      setBusy(false);
    }
  };

  const refresh = useCallback(() => {
    if (active && !writing.current) return reload();
  }, [active, reload]);
  const reread = () => {
    void refresh()?.then(refreshed => { if (refreshed) setActionError(null); });
  };
  const error = [actionError, readError].filter(Boolean).join("。") || null;
  return { profile: readError === null ? profile ?? null : null, lastProfile: profile ?? null,
    loaded: profile !== undefined || readError !== null, busy, error, run, reread, refresh };
}
