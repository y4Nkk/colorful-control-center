import { useCallback, useEffect, useRef, useState } from "react";
import { readBatteryProfile, type BatteryChargeValues, type BatteryProfile } from "../../api/client";
import { toast } from "../use-toast";

export function useBatterySettings(active: boolean, revision: number) {
  const [profile, setProfile] = useState<BatteryProfile | null>(null);
  const [draft, setDraft] = useState<BatteryChargeValues | null>(null);
  const [busy, setBusy] = useState(false), [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const accepted = useRef<BatteryChargeValues | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<BatteryProfile> | null>(null);
  const reload = useCallback(async (reset: boolean, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readBatteryProfile(); reading.current = pending;
    let next: BatteryProfile;
    try { next = await pending; } catch (e) {
      if (id === generation.current) { setProfile(null); setReadError(String(e)); throw e; } else return;
    }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    const before = accepted.current, current = next.charge?.current ?? null;
    accepted.current = structuredClone(current); setProfile(next); setReadError(null);
    setDraft(previous => reset || !previous || JSON.stringify(previous) === JSON.stringify(before) ? structuredClone(current) : previous);
  }, []);
  useEffect(() => {
    if (!active) return;
    const poll = () => reload(false).catch(() => {});
    void poll(); const timer = window.setInterval(() => void poll(), 5000);
    return () => { window.clearInterval(timer); ++generation.current; reading.current = null; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, message?: string) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try { await reading.current?.catch(() => undefined); await action(); await reload(true, true); if (message) toast({ kind: "success", title: message }); }
    catch (e) { setActionError(String(e)); await reload(false, true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, busy, error: [actionError, readError].filter(Boolean).join("；") || null, run,
    reread: () => { void reload(false).catch(() => {}); }, refreshFacts: () => reload(false).catch(() => {}) };
}
