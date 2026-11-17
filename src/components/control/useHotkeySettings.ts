import { useCallback, useEffect, useRef, useState } from "react";
import { onHotkeyStatus, readHotkeys, type HotkeyProfile, type HotkeySettingsValue } from "../../api/client";
import { toast } from "../use-toast";

export function useHotkeySettings(active: boolean, revision: number) {
  const [profile, setProfile] = useState<HotkeyProfile | null>(null), [draft, setDraft] = useState<HotkeySettingsValue | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), accepted = useRef<HotkeySettingsValue | null>(null);
  const reading = useRef<Promise<HotkeyProfile> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readHotkeys(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current;
      accepted.current = next.current;
      setProfile(next);
      setDraft(previous => reset || previous === null || JSON.stringify(previous) === JSON.stringify(before) ? structuredClone(next.current) : previous);
    } catch (error) {
      if (id === generation.current) { setProfile(null); setError(String(error)); }
      throw error;
    } finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(); })().catch(() => {});
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, reload]);
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    void onHotkeyStatus(status => { if (!disposed) setProfile(previous => previous ? { ...previous, status } : previous); })
      .then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(error => { if (!disposed) setError(String(error)); });
    return () => { disposed = true; unlisten?.(); };
  }, []);
  const run = async (action: () => Promise<void>, message: string, reset: boolean) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(reset, true); toast({ kind: "success", title: message });
    } catch (error) { setError(String(error)); await reload(false, true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, busy, error, run,
    reread: () => { setError(null); if (!writing.current) void reload().catch(() => {}); } };
}
