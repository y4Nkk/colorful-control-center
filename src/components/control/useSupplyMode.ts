import { useCallback, useEffect, useRef, useState } from "react";
import { enableOemSupplyMode, pauseOemSupplyMode, readOemSupplyMode, restoreOemSupplyMode, resumeOemSupplyMode, type SupplyModeStatus } from "../../api/client";

export function useSupplyMode(active: boolean, revision: number, onSettled: () => void) {
  const [view, setView] = useState<SupplyModeStatus | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const [pausing, setPausing] = useState(false), [pauseRequested, setPauseRequested] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const writing = useRef(false), stopping = useRef(false), visible = useRef(false), generation = useRef(0);
  const reading = useRef<Promise<void> | null>(null);
  const reload = useCallback(async (afterWrite = false) => {
    if (!visible.current || (!afterWrite && (writing.current || stopping.current))) return;
    if (reading.current) {
      if (!afterWrite) return;
      await reading.current;
      if (!visible.current) return;
    }
    const id = ++generation.current;
    const request = (async () => {
      try {
        const value = await readOemSupplyMode();
        if (id === generation.current) { setView(value); setReadError(null); }
      } catch (error) {
        if (id === generation.current) { setView(null); setReadError(String(error)); }
      }
    })();
    reading.current = request;
    try { await request; }
    finally { if (reading.current === request) reading.current = null; }
  }, []);
  useEffect(() => {
    visible.current = active;
    if (!active) return;
    void reload();
    const timer = setInterval(() => { void reload(); }, 1000);
    return () => { clearInterval(timer); visible.current = false; ++generation.current; };
  }, [active, revision, reload]);
  const run = async (action: () => Promise<void>) => {
    if (writing.current || stopping.current) return false;
    writing.current = true; ++generation.current; setBusy(true); setPauseRequested(false); setError(null);
    try { await action(); await reload(true); return true; }
    catch (error) { await reload(true); setError(String(error)); return false; }
    finally { writing.current = false; setBusy(false); onSettled(); }
  };
  const pause = async () => {
    if (stopping.current) return;
    stopping.current = true; ++generation.current; setPausing(true); setError(null);
    try { await pauseOemSupplyMode(); setPauseRequested(true); }
    catch (error) { setError(String(error)); }
    finally {
      stopping.current = false; setPausing(false);
      if (!writing.current) await reload(true);
      onSettled();
    }
  };
  const confirmed = (action: (expected: string) => Promise<void>) => view ? run(() => action(view.revision)) : Promise.resolve(false);
  return { view, busy, pausing, pauseRequested, error: error ?? readError,
    reread: () => { setError(null); return reload(); }, enable: () => confirmed(enableOemSupplyMode),
    resume: () => confirmed(resumeOemSupplyMode), restore: () => confirmed(restoreOemSupplyMode), pause };
}
