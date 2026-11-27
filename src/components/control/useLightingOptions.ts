import { useCallback, useEffect, useRef, useState } from "react";
import { readLightingOptions, type LightingOptionsProfile } from "../../api/client";
import { toast } from "../use-toast";
import { idleTimerDraft, type LightingOptionsDraft } from "./lighting-option-values";
import { preserveEditedFields } from "./settings-draft";

export function useLightingOptions(active: boolean, revision: number) {
  const [profile, setProfile] = useState<LightingOptionsProfile | null>();
  const [draft, setDraft] = useState<LightingOptionsDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null), [actionError, setActionError] = useState<string | null>(null);
  const accepted = useRef<LightingOptionsDraft | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<LightingOptionsProfile> | null>(null);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readLightingOptions(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current;
      const nextDraft = { ...structuredClone(next.current), idleTimer: idleTimerDraft(next.current.idleTimer) };
      accepted.current = nextDraft;
      setProfile(next); setReadError(null);
      setDraft(previous => reset || !previous || !before ? nextDraft : reconcileOptions(before, previous, nextDraft));
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
  const run = async (action: () => Promise<void>, title: string, reset = true) => {
    if (!active || writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(reset, true);
      toast({ kind: "success", title });
    } catch (error) {
      setActionError(String(error));
      await reload(false, true).catch(() => {});
    } finally { writing.current = false; setBusy(false); }
  };
  const reread = () => { if (!writing.current) void reload().catch(() => {}); };
  return { profile: profile ?? null, loaded: profile !== undefined, draft, setDraft, busy,
    error: [actionError, readError].filter(Boolean).join("；") || null, run, reread };
}

function reconcileOptions(before: LightingOptionsDraft, draft: LightingOptionsDraft, current: LightingOptionsDraft): LightingOptionsDraft {
  const keep = <T extends object>(baseline: T | null, editing: T | null, reported: T | null) =>
    baseline && editing && reported ? preserveEditedFields(baseline, editing, reported) : reported;
  return {
    idleTimer: preserveEditedFields(before.idleTimer, draft.idleTimer, current.idleTimer),
    bootOverride: current.bootOverride,
    logo: keep(before.logo, draft.logo, current.logo),
    lightBar: keep(before.lightBar, draft.lightBar, current.lightBar),
    topCover: keep(before.topCover, draft.topCover, current.topCover),
  };
}
