import { useCallback, useEffect, useRef, useState } from "react";
import { readBatteryCalibration, type BatteryCalibrationProfile } from "../../api/client";
import { useOemConflict } from "../OemConflictHost";

export function useBatteryCalibration(active: boolean, onChanged: () => void, onBusy: (busy: boolean) => void) {
  const conflict = useOemConflict();
  const [profile, setProfile] = useState<BatteryCalibrationProfile | null>(null);
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<BatteryCalibrationProfile | null> | null>(null);
  const reload = useCallback(async (afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readBatteryCalibration(); reading.current = pending;
    let next: BatteryCalibrationProfile | null;
    try { next = await pending; } catch (e) { if (id === generation.current) throw e; else return; }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    setProfile(next); setLoaded(true); setReadError(null);
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload().catch(e => setReadError(String(e)));
    return () => { ++generation.current; reading.current = null; };
  }, [active, reload]);
  const hasProfile = !!profile;
  useEffect(() => {
    if (!active || !hasProfile) return;
    const timer = window.setInterval(() => { void reload().catch(e => setReadError(String(e))); }, 1000);
    return () => window.clearInterval(timer);
  }, [active, hasProfile, reload]);
  const run = async (action: (() => Promise<void>) | null) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null); onBusy(true);
    try { await reading.current?.catch(() => undefined); await action?.(); await reload(true); }
    catch (e) {
      const failure = String(e);
      try { await reload(true); setActionError(failure); }
      catch (readFailure) { setReadError(String(readFailure)); setActionError(`${failure}；重新读取：${readFailure}`); }
      if (action) conflict.review([failure], reread);
    } finally { writing.current = false; setBusy(false); onBusy(false); onChanged(); }
  };
  const reread = () => run(null);
  return { profile, loaded, busy, error: actionError ?? readError, stale: !!readError, run, reread };
}
