import { useCallback, useEffect, useRef, useState } from "react";
import { applyPerkeyLighting, readPerkeyLighting, restorePerkeyLighting, type PerkeyPattern, type PerkeyProfile, type PerkeyValues } from "../../api/client";
import { toast } from "../use-toast";
import { preserveEditedFields } from "./settings-draft";

export function usePerkeyLighting(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<PerkeyProfile | null>(null);
  const [draft, setDraft] = useState<PerkeyValues | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0), accepted = useRef<PerkeyValues | null>(null);
  const patternDrafts = useRef(new Map<PerkeyPattern["kind"], { before: PerkeyPattern; edited: PerkeyPattern }>());
  const reading = useRef<Promise<PerkeyProfile> | null>(null);
  const reload = useCallback(async (reset: boolean, afterWrite = false) => {
    if (reading.current || running.current && !afterWrite) return;
    const id = ++generation.current;
    let next: PerkeyProfile;
    const pending = readPerkeyLighting(); reading.current = pending;
    try { next = await pending; } catch (e) { if (id === generation.current) { setProfile(null); throw e; } else return; }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    setProfile(next);
    const before = accepted.current; accepted.current = next.current;
    const cached = patternDrafts.current.get(next.current.pattern.kind);
    setDraft(previous => {
      const current = next.current;
      if (reset || !previous || !before) return structuredClone(current);
      const values = preserveEditedFields(before, previous, current);
      const kept = before.pattern.kind === current.pattern.kind && previous.pattern.kind === current.pattern.kind
        && usesCustomColors(before.pattern) === usesCustomColors(current.pattern);
      values.pattern = kept ? preserveEditedFields(before.pattern, previous.pattern, current.pattern)
        : before.pattern.kind !== current.pattern.kind && cached && usesCustomColors(cached.before) === usesCustomColors(current.pattern)
          ? preserveEditedFields(cached.before, cached.edited, current.pattern) : current.pattern;
      return values;
    });
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
  const run = async (action: () => Promise<void>, reset = true) => {
    if (!active || running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(reset, true); toast({ kind: "success", title: "逐键灯光已回读" });
    }
    catch (e) {
      setError(String(e));
      await reload(false, true).catch(readError => setError(previous => `${previous}；回读失败：${readError}`));
    }
    finally { running.current = false; setBusy(false); }
  };
  const request = profile && draft ? { expectedRevision: profile.revision, values: draft } : null;
  const needsEvents = draft?.enabled && ["ripple", "fn", "fade"].includes(draft.pattern.kind);
  const dirty = !!profile && !!draft && (JSON.stringify(draft) !== JSON.stringify(profile.current) || !!needsEvents && !profile.reactiveRunning);
  const change = (patch: Partial<PerkeyValues>) => {
    if (!profile || !active || running.current) return;
    const before = profile.current.pattern;
    if (patch.pattern && patch.pattern.kind !== before.kind && draft?.pattern.kind === before.kind) {
      if (JSON.stringify(before) !== JSON.stringify(draft.pattern)) patternDrafts.current.set(before.kind, { before, edited: draft.pattern });
      else patternDrafts.current.delete(before.kind);
    }
    return run(() => applyPerkeyLighting({ expectedRevision: profile.revision, values: { ...profile.current, ...patch } }), false);
  };
  return { profile, draft, setDraft, busy, error, request, dirty,
    reload: () => reload(false).catch(e => setError(String(e))),
    change,
    apply: () => request ? run(async () => {
      await applyPerkeyLighting(request);
      patternDrafts.current.delete(request.values.pattern.kind);
    }) : undefined,
    restore: () => run(async () => {
      await restorePerkeyLighting();
      patternDrafts.current.clear();
    }) };
}

function usesCustomColors(pattern: PerkeyValues["pattern"]) {
  return "colors" in pattern ? pattern.colors !== null : pattern.color !== null;
}
