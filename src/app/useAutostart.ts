import { useCallback, useEffect, useRef, useState } from "react";
import { getAutostart, setAutostart, toCommandError, type AutostartSnapshot, type AutostartLevel } from "../api/client";

export function useAutostart(active: boolean) {
  const [snapshot, setSnapshot] = useState<AutostartSnapshot | null>(null);
  const [level, setLevel] = useState<AutostartLevel>("disabled");
  const [pending, setPending] = useState<"read" | "write" | null>("read");
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(false);
  const inFlight = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const request = useCallback(async (operation: () => Promise<AutostartSnapshot>, writing: boolean) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setPending(writing ? "write" : "read"); setError(null);
    try {
      const next = await operation();
      if (mounted.current) { setSnapshot(next); setLevel(next.level); }
    } catch (cause) {
      if (mounted.current) { setError(toCommandError(cause).message); setSnapshot(null); }
    } finally {
      inFlight.current = false;
      if (mounted.current) setPending(null);
    }
  }, []);
  const reload = useCallback(() => request(getAutostart, false), [request]);
  useEffect(() => { if (active) void reload(); }, [active, reload]);
  const apply = (restore = false) => snapshot
    ? request(() => setAutostart(snapshot.revision, level, restore), true) : Promise.resolve();
  return { snapshot, level, setLevel, pending, error, reload, apply };
}
