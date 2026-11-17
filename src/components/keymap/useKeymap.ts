import { useCallback, useEffect, useRef, useState } from "react";
import { readKeymap, type KeymapConfiguration, type KeymapProfile } from "../../api/client";
import { toast } from "../use-toast";

export function useKeymap(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<KeymapProfile | null>(null), [draft, setDraft] = useState<KeymapConfiguration | null>(null);
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const writing = useRef(false), reading = useRef<Promise<KeymapProfile | null> | null>(null), generation = useRef(0);
  const accepted = useRef<KeymapConfiguration | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readKeymap(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setProfile(next); setLoaded(true); setReadError(null);
      const before = accepted.current; accepted.current = next?.configuration ?? null;
      setDraft(previous => reset || previous === null || JSON.stringify(previous) === JSON.stringify(before)
        ? next?.configuration ? structuredClone(next.configuration) : null : previous);
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
  const run = async (action: () => Promise<void>, title: string, resetDraft = true) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try { await reading.current?.catch(() => undefined); await action(); await reload(resetDraft, true); toast({ kind: "success", title }); }
    catch (error) { setActionError(String(error)); await reload(false, true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, loaded, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run,
    reread: () => { if (!writing.current) void reload().catch(() => {}); } };
}
