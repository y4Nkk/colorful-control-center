import { useCallback, useEffect, useRef, useState } from "react";
import { readUsbPolicy, type UsbProfile, type UsbTargets } from "../../api/client";
import { toast } from "../use-toast";

export function useUsbPowerPolicy(active: boolean, revision: number) {
  const [profile, setProfile] = useState<UsbProfile | null>(null), [draft, setDraft] = useState<UsbTargets>({ ac: null, dc: null });
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<UsbProfile | null> | null>(null);
  const reload = useCallback(async (afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readUsbPolicy(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setProfile(next); setLoaded(true); setReadError(null);
    } catch (error) {
      if (id === generation.current) { setProfile(null); setLoaded(true); setReadError(String(error)); }
      throw error;
    } finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(); })().catch(() => {});
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, title: string, reset = false) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined); await action(); await reload(true);
      if (reset) setDraft({ ac: null, dc: null }); toast({ kind: "success", title });
    } catch (error) { setActionError(String(error)); await reload(true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, loaded, busy, run, error: [actionError, readError].filter(Boolean).join("；") || null,
    reread: () => { if (!writing.current) void reload().catch(() => {}); } };
}
