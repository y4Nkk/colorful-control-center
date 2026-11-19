import { useCallback, useEffect, useRef, useState } from "react";
import { readFanDust, type FanDustProfile } from "../../api/client";
import { toast } from "../use-toast";

export function useFanDust(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<FanDustProfile | null>(null);
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<FanDustProfile | null> | null>(null);
  const observed = useRef<boolean | null>(null), settled = useRef(onSettled);
  useEffect(() => { settled.current = onSettled; }, [onSettled]);
  const reload = useCallback(async (afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readFanDust(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setProfile(next); setReadError(null); setLoaded(true);
      if (next && observed.current !== null && observed.current !== next.running) settled.current();
      observed.current = next?.running ?? null;
    } catch (error) {
      if (id === generation.current) { setProfile(null); setReadError(String(error)); setLoaded(true); }
      throw error;
    } finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload().catch(() => {});
    const timer = setInterval(() => { void reload().catch(() => {}); }, 1000);
    return () => { clearInterval(timer); ++generation.current; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, title: string) => {
    if (!active || writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(true); settled.current();
      toast({ kind: "success", title });
    } catch (error) {
      setActionError(String(error)); await reload(true).catch(() => {});
    } finally { writing.current = false; setBusy(false); }
  };
  const reread = () => { if (!writing.current) void reload().catch(() => {}); };
  return { profile, loaded, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run, reread };
}
