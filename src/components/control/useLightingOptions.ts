import { useCallback, useEffect, useRef, useState } from "react";
import { readLightingOptions, type LightingOptionsProfile, type LightingOptionsValues } from "../../api/client";
import { toast } from "../use-toast";

export function useLightingOptions(active: boolean, revision: number) {
  const [profile, setProfile] = useState<LightingOptionsProfile | null>(null);
  const [draft, setDraft] = useState<LightingOptionsValues | null>(null);
  const [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const accepted = useRef<LightingOptionsValues | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<LightingOptionsProfile> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readLightingOptions(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current; accepted.current = structuredClone(next.current);
      setProfile(next); setReadError(null);
      setDraft(previous => reset || !previous || JSON.stringify(previous) === JSON.stringify(before) ? structuredClone(next.current) : {
        ...previous,
        bootOverride: next.current.bootOverride === null ? null : previous.bootOverride ?? next.current.bootOverride,
        logo: next.current.logo === null ? null : previous.logo ?? structuredClone(next.current.logo),
        lightBar: next.current.lightBar === null ? null : previous.lightBar ?? structuredClone(next.current.lightBar),
        topCover: next.current.topCover === null ? null : previous.topCover ?? structuredClone(next.current.topCover),
      });
    } catch (error) {
      if (id === generation.current) { setProfile(null); setReadError(String(error)); }
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
      setActionError(String(error));
      await reload(false, true).catch(() => {});
    } finally { writing.current = false; setBusy(false); }
  };
  const reread = () => { if (!writing.current) void reload().catch(() => {}); };
  return { profile, draft, setDraft, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run, reread };
}
