import { useCallback, useEffect, useRef, useState } from "react";
import { applyPerkeyLighting, readPerkeyLighting, restorePerkeyLighting, type PerkeyPattern, type PerkeyProfile, type PerkeyValues } from "../../api/client";
import { toast } from "../use-toast";
import { preserveEditedFields } from "./settings-draft";

type PatternDraft = { before: PerkeyPattern; edited: PerkeyPattern };

export function usePerkeyLighting(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<PerkeyProfile | null>();
  const [draft, setDraft] = useState<PerkeyValues | null>(null);
  const [busy, setBusy] = useState(false), [actionError, setActionError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0), accepted = useRef<PerkeyValues | null>(null);
  const patternDrafts = useRef(new Map<PerkeyPattern["kind"], PatternDraft>());
  const reading = useRef<Promise<PerkeyProfile> | null>(null);
  const reload = useCallback(async (reset: boolean, afterWrite = false) => {
    if (reading.current || running.current && !afterWrite) return;
    const id = ++generation.current;
    let next: PerkeyProfile;
    const pending = readPerkeyLighting(); reading.current = pending;
    try { next = await pending; }
    catch (error) {
      if (id !== generation.current) return;
      setProfile(null); setReadError(String(error)); throw error;
    }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    setProfile(next); setReadError(null);
    const before = accepted.current; accepted.current = next.current;
    const cached = patternDrafts.current.get(next.current.pattern.kind);
    setDraft(previous => reset || !previous || !before
      ? structuredClone(next.current) : reconcilePerkeyDraft(before, previous, next.current, cached));
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await reload(false); })().catch(() => {});
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, reload]);
  const reactive = !!profile && profile.current.enabled && ["ripple", "fn", "fade"].includes(profile.current.pattern.kind);
  useEffect(() => {
    if (!active || !reactive) return;
    const timer = setInterval(() => { void reload(false).catch(() => {}); }, 3000);
    return () => clearInterval(timer);
  }, [active, reactive, reload]);
  const run = async (action: () => Promise<void>, reset = true) => {
    if (!active || running.current) return;
    running.current = true; ++generation.current; setBusy(true); setActionError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(reset, true); toast({ kind: "success", title: "逐键灯光已回读" });
    }
    catch (e) {
      setActionError(String(e));
      await reload(false, true).catch(() => {});
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
  return { profile: profile ?? null, loaded: profile !== undefined, draft, setDraft, busy, request, dirty,
    error: actionError && readError ? `${actionError}；回读失败：${readError}` : actionError ?? readError,
    reload: () => reload(false).catch(() => {}),
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

function reconcilePerkeyDraft(before: PerkeyValues, draft: PerkeyValues, current: PerkeyValues, cached?: PatternDraft): PerkeyValues {
  const values = preserveEditedFields(before, draft, current);
  const samePattern = before.pattern.kind === current.pattern.kind && draft.pattern.kind === current.pattern.kind
    && usesCustomColors(before.pattern) === usesCustomColors(current.pattern);
  values.pattern = samePattern ? preserveEditedFields(before.pattern, draft.pattern, current.pattern)
    : before.pattern.kind !== current.pattern.kind && cached && usesCustomColors(cached.before) === usesCustomColors(current.pattern)
      ? preserveEditedFields(cached.before, cached.edited, current.pattern) : current.pattern;
  return values;
}

function usesCustomColors(pattern: PerkeyValues["pattern"]) {
  return "colors" in pattern ? pattern.colors !== null : pattern.color !== null;
}
