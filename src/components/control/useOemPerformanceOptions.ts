import { useCallback, useEffect, useRef, useState } from "react";
import { readOemPerformanceOptions, type OemPerformanceOptions, type OemPerformanceValues, type OpResult } from "../../api/client";
import { toast } from "../use-toast";
export function useOemPerformanceOptions(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<OemPerformanceOptions | null>(null), [draft, setDraft] = useState<OemPerformanceValues | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const accepted = useRef<OemPerformanceValues | null>(null);
  const generation = useRef(0), running = useRef(false);
  const reload = useCallback(async (reset: boolean) => {
    const id = ++generation.current;
    let next: OemPerformanceOptions;
    try { next = await readOemPerformanceOptions(); } catch (e) { if (id === generation.current) { setProfile(null); throw e; } else return; }
    if (id !== generation.current) return;
    const before = accepted.current;
    accepted.current = next.current;
    setProfile(next);
    setDraft(previous => {
      if (reset || !previous || !before) return next.current;
      return {
        turboFan: reconcileOption(next.current.turboFan, previous.turboFan, before.turboFan),
        dtt: reconcileOption(next.current.dtt, previous.dtt, before.dtt),
      };
    });
  }, []);
  useEffect(() => { if (active) void reload(false).catch(e => setError(String(e))); return () => { ++generation.current; }; }, [active, revision, reload]);
  const run = async (action: () => Promise<OpResult>, message: string) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { const result = await action(); if (!result.ok) throw new Error(result.message); await reload(true); toast({ kind: "success", title: message }); }
    catch (e) { setError(String(e)); await reload(false).catch(e => setError(previous => `${previous}；回读失败：${e}`)); }
    finally { running.current = false; setBusy(false); onSettled(); }
  };
  const reread = async () => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await reload(false); } catch (e) { setError(String(e)); } finally { running.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, busy, error, run, reread };
}

function reconcileOption(current: boolean | null, draft: boolean | null, before: boolean | null) {
  return current !== null && draft !== null && draft !== before ? draft : current;
}
