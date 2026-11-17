import { useCallback, useEffect, useRef, useState } from "react";
import { readMultiZoneLighting, type MultiZoneProfile, type MultiZoneTarget } from "../../api/client";
import { toast } from "../use-toast";

export function useMultiZoneLighting(active: boolean, revision: number) {
  const [profile, setProfile] = useState<MultiZoneProfile | null>(null), [draft, setDraft] = useState<MultiZoneTarget | null>(null);
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), accepted = useRef<MultiZoneTarget | null>(null);
  const reading = useRef<Promise<MultiZoneProfile | null> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readMultiZoneLighting(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current;
      const target = next?.current ? structuredClone({ capabilities: next.capabilities, values: next.current }) : null;
      accepted.current = target;
      setProfile(next); setLoaded(true); setReadError(null);
      setDraft(previous => reset || !previous || JSON.stringify(previous) === JSON.stringify(before) ? structuredClone(target) : previous);
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
  const run = async (action: () => Promise<void>, message: string) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try { await reading.current?.catch(() => undefined); await action(); await reload(true, true); toast({ kind: "success", title: message }); }
    catch (error) { setActionError(String(error)); await reload(false, true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, loaded, busy, run, error: [actionError, readError].filter(Boolean).join("；") || null,
    reread: () => { if (!writing.current) void reload().catch(() => {}); } };
}
