import { useCallback, useEffect, useRef, useState } from "react";
import { readPlatformPower, type PlatformPowerProfile } from "../../api/client";
import { toast } from "../use-toast";

export function usePlatformPower(active: boolean, revision: number) {
  const [profile, setProfile] = useState<PlatformPowerProfile | null>(null), [draft, setDraft] = useState<number | null>(null);
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<PlatformPowerProfile> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readPlatformPower(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setProfile(next); setReadError(null); setLoaded(true);
      setDraft(previous => reset || previous === null ? next.powerDelivery?.currentLevel ?? null : previous);
    } catch (error) {
      if (id === generation.current) { setProfile(null); setReadError(String(error)); setLoaded(true); }
      throw error;
    } finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(); })().catch(() => {});
    const timer = setInterval(() => { void reload().catch(() => {}); }, 5000);
    return () => { disposed = true; clearInterval(timer); ++generation.current; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, title: string) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(true, true);
      toast({ kind: "success", title });
    } catch (error) {
      setActionError(String(error)); await reload(false, true).catch(() => {});
    } finally { writing.current = false; setBusy(false); }
  };
  const reread = () => { if (!writing.current) void reload().catch(() => {}); };
  return { profile, draft, setDraft, loaded, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run, reread };
}
