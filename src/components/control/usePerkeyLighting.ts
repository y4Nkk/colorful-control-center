import { useCallback, useEffect, useRef, useState } from "react";
import { applyPerkeyLighting, readPerkeyLighting, restorePerkeyLighting, type PerkeyProfile, type PerkeyValues } from "../../api/client";
import { toast } from "../use-toast";

export function usePerkeyLighting(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<PerkeyProfile | null>(null);
  const [draft, setDraft] = useState<PerkeyValues | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0), accepted = useRef<PerkeyValues | null>(null);
  const reading = useRef<Promise<PerkeyProfile> | null>(null);
  const reload = useCallback(async (reset: boolean) => {
    if (reading.current || running.current && !reset) return;
    const id = ++generation.current;
    let next: PerkeyProfile;
    const pending = readPerkeyLighting(); reading.current = pending;
    try { next = await pending; } catch (e) { if (id === generation.current) { setProfile(null); throw e; } else return; }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    setProfile(next);
    const before = accepted.current; accepted.current = next.current;
    setDraft(previous => reset || previous === null || JSON.stringify(previous) === JSON.stringify(before) ? structuredClone(next.current) : previous);
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(false); })().catch(e => setError(String(e)));
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, reload]);
  const reactive = !!profile && profile.current.enabled && ["ripple", "fn", "fade"].includes(profile.current.pattern.kind);
  useEffect(() => {
    if (!active || !reactive) return;
    const timer = setInterval(() => { void reload(false).catch(e => setError(String(e))); }, 3000);
    return () => clearInterval(timer);
  }, [active, reactive, reload]);
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(true); toast({ kind: "success", title: "逐键灯光已写入" });
    }
    catch (e) {
      setError(String(e));
      running.current = false;
      await reload(false).catch(readError => setError(previous => `${previous}；回读失败：${readError}`));
    }
    finally { running.current = false; setBusy(false); }
  };
  const request = profile && draft ? { expectedRevision: profile.revision, values: draft } : null;
  const needsEvents = draft?.enabled && ["ripple", "fn", "fade"].includes(draft.pattern.kind);
  const dirty = !!profile && !!draft && (JSON.stringify(draft) !== JSON.stringify(profile.current) || !!needsEvents && !profile.reactiveRunning);
  return { profile, draft, setDraft, busy, error, request, dirty,
    reload: () => reload(false).catch(e => setError(String(e))),
    apply: () => request ? run(() => applyPerkeyLighting(request)) : undefined,
    restore: () => run(restorePerkeyLighting) };
}
