import { useCallback, useEffect, useRef, useState } from "react";
import {
  applySavedCpu, applyRuntimeCpu,
  readPowerProfile, restorePowerProfile, type PowerProfile, type PowerValues,
} from "../../api/client";
import { toast } from "../use-toast";
import type { DiffItem } from "./DiffPreview";

export type PowerTarget = "runtime" | "saved";
export interface PowerDraft { pl1: number; pl2: number; coreMv: number; gtMv: number }
const valuesOf = (profile: PowerProfile, target: PowerTarget): PowerValues => target === "runtime" ? profile.runtime : profile;
const draftOf = (values: PowerValues): PowerDraft => ({
  pl1: Math.round(values.pl1W ?? 35), pl2: Math.round(values.pl2W ?? 157),
  coreMv: Math.round(values.coreOffsetMv ?? 0), gtMv: Math.round(values.gtOffsetMv ?? 0),
});

export function powerChanges(values: PowerValues, draft: PowerDraft) {
  const current = draftOf(values);
  const plReady = values.pl1W != null && values.pl2W != null;
  const coreReady = values.coreOffsetMv != null;
  const gtReady = values.gtOffsetMv != null;
  const pl = plReady && (current.pl1 !== draft.pl1 || current.pl2 !== draft.pl2);
  const core = coreReady && current.coreMv !== draft.coreMv;
  const gt = gtReady && current.gtMv !== draft.gtMv;
  const items: DiffItem[] = [];
  if (pl) items.push({ label: "持续功耗上限", from: `${current.pl1} W`, to: `${draft.pl1} W` },
    { label: "短时功耗上限", from: `${current.pl2} W`, to: `${draft.pl2} W` });
  if (core) items.push({ label: "核心电压偏移", from: `${current.coreMv} mV`, to: `${draft.coreMv} mV` });
  if (gt) items.push({ label: "核显电压偏移", from: `${current.gtMv} mV`, to: `${draft.gtMv} mV` });
  return { current, plReady, coreReady, gtReady, pl, core, gt, items };
}

async function applyDraft(values: PowerValues, draft: PowerDraft, target: PowerTarget) {
  const dirty = powerChanges(values, draft);
  const result = await (target === "runtime" ? applyRuntimeCpu : applySavedCpu)({
    pl1W: dirty.pl ? draft.pl1 : null, pl2W: dirty.pl ? draft.pl2 : null,
    coreOffsetMv: dirty.core ? draft.coreMv : null, gtOffsetMv: dirty.gt ? draft.gtMv : null,
  });
  if (!result.ok) throw new Error(result.message);
}

export function usePowerControl(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<PowerProfile | null>(null);
  const [drafts, setDrafts] = useState<Record<PowerTarget, PowerDraft | null>>({ runtime: null, saved: null });
  const [target, setTarget] = useState<PowerTarget>("runtime");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const previous = useRef<PowerProfile | null>(null);
  const loadId = useRef(0);
  const load = useCallback(async (reset: boolean) => {
    const id = ++loadId.current;
    const next = await readPowerProfile();
    if (id !== loadId.current) return;
    const before = previous.current;
    setDrafts((current) => Object.fromEntries((["runtime", "saved"] as const).map((key) => {
      const draft = current[key];
      const dirty = before && draft && powerChanges(valuesOf(before, key), draft).items.length > 0;
      return [key, !reset && dirty ? draft : draftOf(valuesOf(next, key))];
    })) as Record<PowerTarget, PowerDraft>);
    if (!before && !next.msrRuntimeAvailable) setTarget("saved");
    previous.current = next; setProfile(next); setError(null);
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void load(false).catch((cause) => { if (!disposed) setError(String(cause)); });
    return () => { disposed = true; ++loadId.current; };
  }, [active, revision, load]);
  const run = async (action: () => Promise<void>, success: string) => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await action(); await load(true); toast({ kind: "success", title: success }); }
    catch (cause) {
      const message = String(cause);
      try { await load(false); setError(message); }
      catch (refreshError) { setError(`${message}；回读失败：${String(refreshError)}`); }
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const draft = drafts[target];
  const values = profile ? valuesOf(profile, target) : null;
  const dirty = values && draft ? powerChanges(values, draft) : null;
  const change = (patch: Partial<PowerDraft>) => setDrafts((current) => {
    const value = current[target];
    if (!value || running.current) return current;
    const next = { ...value, ...patch };
    if (patch.pl1 != null) next.pl2 = Math.max(next.pl1, next.pl2);
    if (patch.pl2 != null) next.pl1 = Math.min(next.pl1, next.pl2);
    return { ...current, [target]: next };
  });
  const apply = () => values && draft ? run(() => applyDraft(values, draft, target),
    target === "saved" ? "设置已保存" : "实时值已回读确认") : Promise.resolve();
  const restore = () => run(async () => {
    const result = await restorePowerProfile();
    if (!result.ok) throw new Error(result.message);
  }, "功耗与电压已恢复");
  return { profile, draft, target, setTarget, error, busy, dirty, change, apply, restore,
    retry: () => { void load(false).catch((cause) => setError(String(cause))); } };
}
