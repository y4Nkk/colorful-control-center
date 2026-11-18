import { useCallback, useEffect, useRef, useState } from "react";
import { readAirplane, type AirplaneProfile } from "../../api/client";
import { toast } from "../use-toast";

export function useAirplaneSettings(active: boolean, revision: number) {
  const [profile, setProfile] = useState<AirplaneProfile | null>(null);
  const [choice, setChoice] = useState("keep");
  const [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<AirplaneProfile> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readAirplane(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setProfile(next); setReadError(null);
      if (reset) setChoice("keep");
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
    if (writing.current) return false;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(true, true);
      toast({ kind: "success", title });
      return true;
    } catch (error) {
      setActionError(String(error));
      await reload(false, true).catch(() => {});
      return false;
    } finally { writing.current = false; setBusy(false); }
  };
  const reread = () => { if (!writing.current) void reload().catch(() => {}); };
  return { profile, choice, setChoice, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run, reread };
}

