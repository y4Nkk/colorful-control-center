import { useCallback, useEffect, useRef, useState } from "react";
import { previewLanPolicy, readLanPolicy, type LanProfile, type LanRequest, type LanChange } from "../../api/client";
import { toast } from "../use-toast";
interface Draft { deviceId: string; powerSaving: boolean | null; applyWakePatch: boolean }
function useLanPreview(request: LanRequest | null, active: boolean, busy: boolean, snapshot: LanProfile | null) {
  const key = JSON.stringify(request), [result, setResult] = useState<{ key: string; changes: LanChange[]; error: string | null } | null>(null);
  const pending = useRef<Promise<LanChange[]> | null>(null);
  useEffect(() => {
    if (!active || busy || !request || request.powerSaving === null && !request.applyWakePatch) { setResult(null); return; }
    let disposed = false;
    void (async () => {
      await pending.current?.catch(() => undefined); if (disposed) return;
      const promise = previewLanPolicy(request); pending.current = promise;
      try { const changes = await promise; if (!disposed) setResult({ key, changes, error: null }); }
      catch (error) { if (!disposed) setResult({ key, changes: [], error: String(error) }); }
      finally { if (pending.current === promise) pending.current = null; }
    })();
    return () => { disposed = true; };
  }, [key, active, busy, snapshot]);
  return { pending, changes: result?.key === key ? result.changes : [], error: result?.key === key ? result.error : null,
    ready: result?.key === key && !result.error };
}
export function useLanPolicy(active: boolean, revision: number) {
  const [profile, setProfile] = useState<LanProfile | null>(null), [draft, setDraft] = useState<Draft>({ deviceId: "", powerSaving: null, applyWakePatch: false });
  const [loaded, setLoaded] = useState(false), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const reading = useRef<Promise<LanProfile | null> | null>(null), writing = useRef(false), generation = useRef(0);
  const reload = useCallback(async (afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readLanPolicy(); reading.current = pending;
    try {
      const next = await pending; if (id !== generation.current) return;
      setProfile(next); setLoaded(true); setReadError(null);
      setDraft(value => value.deviceId ? value : { ...value, deviceId: next?.frame.devices[0]?.identity.instance ?? next?.initial[0]?.identity.instance ?? "" });
    } catch (error) { if (id === generation.current) { setProfile(null); setLoaded(true); setReadError(String(error)); } throw error; }
    finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return; let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(); })().catch(() => {});
    return () => {
      disposed = true;
      if (!writing.current) { ++generation.current; reading.current = null; }
    };
  }, [active, revision, reload]);
  const request: LanRequest | null = profile && draft.deviceId ? { expectedRevision: profile.revision, ...draft } : null;
  const preview = useLanPreview(request, active, busy, profile);
  const run = async (action: () => Promise<void>, title: string, reset = false) => {
    if (writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined); await preview.pending.current?.catch(() => undefined);
      await action(); await reload(true);
      if (reset) setDraft(value => ({ ...value, powerSaving: null, applyWakePatch: false })); toast({ kind: "success", title });
    } catch (error) { setActionError(String(error)); await reload(true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  return { profile, draft, setDraft, loaded, busy, request, preview, run,
    error: [actionError, readError, preview.error].filter(Boolean).join("；") || null,
    reread: () => { if (!writing.current) void reload().catch(() => {}); } };
}
