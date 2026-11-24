import { useCallback, useEffect, useRef, useState } from "react";
import {
  applySavedCpu, applyRuntimeCpu,
  readPowerProfile, restorePowerProfile, type PowerProfile, type PowerValues,
} from "../../api/client";
import { toast } from "../use-toast";
import type { DiffItem } from "./DiffPreview";

export type PowerTarget = "runtime" | "saved";
export const CPU_PL_MIN = 35;
export const CPU_PL_MAX = 157;
interface PowerLimits { pl1: number; pl2: number }
interface PowerDraft { limits: PowerLimits | null; coreMv: number; gtMv: number }
const valuesOf = (profile: PowerProfile, target: PowerTarget): PowerValues => target === "runtime" ? profile.runtime : profile;
const sameLimits = (a: PowerLimits | null, b: PowerLimits | null) => a?.pl1 === b?.pl1 && a?.pl2 === b?.pl2;
const editableLimit = (value: number | null) => {
  if (value == null) return null;
  const rounded = Math.round(value);
  return rounded >= CPU_PL_MIN && rounded <= CPU_PL_MAX ? rounded : null;
};

const draftOf = (profile: PowerProfile, target: PowerTarget): PowerDraft => {
  const values = valuesOf(profile, target);
  const pl1 = editableLimit(values.pl1W ?? (target === "saved" ? profile.defaultPl1W : null));
  const pl2 = editableLimit(values.pl2W ?? (target === "saved" ? profile.defaultPl2W : null));
  return {
    limits: pl1 != null && pl2 != null && pl1 <= pl2 ? { pl1, pl2 } : null,
    coreMv: Math.round(values.coreOffsetMv ?? 0), gtMv: Math.round(values.gtOffsetMv ?? 0),
  };
};
const plFrom = (value: number | null | undefined) => value == null ? "未上报" : `${Math.round(value)} W`;

function powerChanges(profile: PowerProfile, target: PowerTarget, draft: PowerDraft) {
  const values = valuesOf(profile, target);
  const current = draftOf(profile, target);
  const coreReady = values.coreOffsetMv != null;
  const gtReady = values.gtOffsetMv != null;
  const limits = current.limits && !sameLimits(current.limits, draft.limits) ? draft.limits : null;
  const core = coreReady && current.coreMv !== draft.coreMv;
  const gt = gtReady && current.gtMv !== draft.gtMv;
  const items: DiffItem[] = [];
  if (limits) items.push({ label: "持续功耗上限", from: plFrom(values.pl1W), to: `${limits.pl1} W` },
    { label: "短时功耗上限", from: plFrom(values.pl2W), to: `${limits.pl2} W` });
  if (core) items.push({ label: "核心电压偏移", from: `${current.coreMv} mV`, to: `${draft.coreMv} mV` });
  if (gt) items.push({ label: "核显电压偏移", from: `${current.gtMv} mV`, to: `${draft.gtMv} mV` });
  return { current, coreReady, gtReady, limits, core, gt, items };
}

async function applyDraft(profile: PowerProfile, target: PowerTarget, draft: PowerDraft) {
  const dirty = powerChanges(profile, target, draft);
  const result = await (target === "runtime" ? applyRuntimeCpu : applySavedCpu)({
    pl1W: dirty.limits?.pl1 ?? null, pl2W: dirty.limits?.pl2 ?? null,
    coreOffsetMv: dirty.core ? draft.coreMv : null, gtOffsetMv: dirty.gt ? draft.gtMv : null,
  });
  if (!result.ok) throw new Error(result.message);
}

export function usePowerControl(active: boolean, revision: number) {
  const [profile, setProfile] = useState<PowerProfile | null>(null);
  const [drafts, setDrafts] = useState<Record<PowerTarget, PowerDraft | null>>({ runtime: null, saved: null });
  const [target, setTarget] = useState<PowerTarget>("runtime");
  const [actionError, setActionError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const previous = useRef<PowerProfile | null>(null);
  const loadId = useRef(0);
  const load = useCallback(async (reset: PowerTarget | "all" | null) => {
    const id = ++loadId.current;
    let next: PowerProfile;
    try { next = await readPowerProfile(); }
    catch (error) { if (id === loadId.current) setReadError(String(error)); return false; }
    if (id !== loadId.current) return false;
    const before = previous.current;
    setDrafts((current) => Object.fromEntries((["runtime", "saved"] as const).map((key) => {
      const draft = current[key];
      const fresh = draftOf(next, key);
      if (!before || !draft || reset === "all" || reset === key) return [key, fresh];
      const changes = powerChanges(before, key, draft);
      return [key, {
        limits: changes.limits && sameLimits(changes.current.limits, fresh.limits) ? draft.limits : fresh.limits,
        coreMv: changes.core ? draft.coreMv : fresh.coreMv,
        gtMv: changes.gt ? draft.gtMv : fresh.gtMv,
      }];
    })) as Record<PowerTarget, PowerDraft>);
    if (!before && !next.msrRuntimeAvailable) setTarget("saved");
    previous.current = next; setProfile(next); setReadError(null);
    return true;
  }, []);
  useEffect(() => {
    if (!active || running.current) return;
    void load(null);
    return () => { if (!running.current) ++loadId.current; };
  }, [active, revision, load]);
  const run = async (action: () => Promise<void>, success: string, reset: PowerTarget | "all") => {
    if (!active || running.current || readError !== null) return;
    running.current = true; ++loadId.current; setBusy(true);
    try {
      await action();
      const refreshed = await load(reset);
      setActionError(null);
      if (refreshed) toast({ kind: "success", title: success });
    } catch (cause) {
      await load(null);
      setActionError(String(cause));
    } finally { running.current = false; setBusy(false); }
  };
  const draft = drafts[target];
  const dirty = profile && draft ? powerChanges(profile, target, draft) : null;
  const change = (patch: Partial<PowerLimits & Omit<PowerDraft, "limits">>) => setDrafts((current) => {
    const value = current[target];
    if (!value || running.current) return current;
    const { pl1, pl2, ...voltage } = patch;
    let limits = value.limits;
    if (limits && pl1 != null) limits = { pl1, pl2: Math.max(pl1, limits.pl2) };
    if (limits && pl2 != null) limits = { pl1: Math.min(pl2, limits.pl1), pl2 };
    return { ...current, [target]: { ...value, ...voltage, limits } };
  });
  const reviews = profile ? (["runtime", "saved"] as const).flatMap(scope => {
    const next = drafts[scope];
    const changes = next && powerChanges(profile, scope, next);
    return next && changes?.items.length && (scope === "saved" || profile.msrRuntimeAvailable)
      ? [{ scope, items: changes.items, apply: () => run(() => applyDraft(profile, scope, next),
        scope === "saved" ? "设置已保存" : "实时值已回读确认", scope) }]
      : [];
  }) : [];
  const restore = () => run(async () => {
    const result = await restorePowerProfile();
    if (!result.ok) throw new Error(result.message);
  }, "功耗与电压已恢复", "all");
  const error = [actionError, readError].filter(Boolean).join("；") || null;
  return { profile, draft, target, setTarget, error, readError, busy, dirty, change, reviews, restore,
    retry: () => { if (active && !running.current) void load(null).then(next => { if (next) setActionError(null); }); } };
}
