import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "../use-toast";

export function useDeviceSettings<T>(read: () => Promise<T>, active: boolean, revision: number | string, pollMs?: number) {
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
        setProfile(null);
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
    setActionError(null);
    try {
      await action();
      if (await reload() && successTitle) toast({ kind: "success", title: successTitle });
    } catch (error) {
      setActionError(String(error));
      await reload();
    } finally {
      writing.current = false;
      setBusy(false);
    }
  };

  const refresh = useCallback(() => {
    if (active && !writing.current) return reload();
  }, [active, reload]);
  const reread = () => {
    setActionError(null);
    void refresh();
  };
  const error = [actionError, readError].filter(Boolean).join("。") || null;
  return { profile: profile ?? null, loaded: profile !== undefined, busy, error, run, reread, refresh };
}
