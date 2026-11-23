import { useCallback, useEffect, useRef, useState } from "react";
import { getAutostart, setAutostart, toCommandError, type AutostartSnapshot, type AutostartLevel } from "../api/client";
import { toast } from "../components/use-toast";

export function useAutostart(active: boolean) {
  const [snapshot, setSnapshot] = useState<AutostartSnapshot | null>(null);
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
      if (mounted.current) setSnapshot(next);
    } catch (cause) {
      if (mounted.current) { setError(toCommandError(cause).message); setSnapshot(null); }
    } finally {
      inFlight.current = false;
      if (mounted.current) setPending(null);
    }
  }, []);
  const reload = useCallback(() => request(getAutostart, false), [request]);
  useEffect(() => { if (active) void reload(); }, [active, reload]);
  const apply = (level: AutostartLevel, restore = false) => active && snapshot && (restore || level !== snapshot.level || snapshot.legacy)
    ? request(async () => {
        const next = await setAutostart(snapshot.revision, level, restore);
        toast({ kind: "success", title: restore ? "已恢复登录自启设置" : "已保存登录自启设置" });
        return next;
      }, true) : Promise.resolve();
  return { snapshot, pending, error, reload, apply };
}
