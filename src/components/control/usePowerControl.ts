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
// PL 保存值未上报（固件未声明 CPUOC、page6 从未写入的机型）时草稿锚定出厂默认，
// 首次调节整组提交 PL1+PL2；即时值在 MSR 可用时总是成对上报，不可用时整组滑杆已禁用。
const draftOf = (profile: PowerProfile, target: PowerTarget): PowerDraft => {
  const values = valuesOf(profile, target);
  const pl1 = values.pl1W ?? (target === "saved" ? profile.defaultPl1W : null);
  const pl2 = values.pl2W ?? (target === "saved" ? profile.defaultPl2W : null);
  return {
    pl1: Math.round(pl1 ?? 35), pl2: Math.round(pl2 ?? 157),
    coreMv: Math.round(values.coreOffsetMv ?? 0), gtMv: Math.round(values.gtOffsetMv ?? 0),
  };
};
const plFrom = (value: number | null | undefined) => value == null ? "未上报" : `${Math.round(value)} W`;

function powerChanges(profile: PowerProfile, target: PowerTarget, draft: PowerDraft) {
  const values = valuesOf(profile, target);
  const current = draftOf(profile, target);
  const coreReady = values.coreOffsetMv != null;
  const gtReady = values.gtOffsetMv != null;
  const pl = current.pl1 !== draft.pl1 || current.pl2 !== draft.pl2;
  const core = coreReady && current.coreMv !== draft.coreMv;
  const gt = gtReady && current.gtMv !== draft.gtMv;
  const items: DiffItem[] = [];
  if (pl) items.push({ label: "持续功耗上限", from: plFrom(values.pl1W), to: `${draft.pl1} W` },
    { label: "短时功耗上限", from: plFrom(values.pl2W), to: `${draft.pl2} W` });
  if (core) items.push({ label: "核心电压偏移", from: `${current.coreMv} mV`, to: `${draft.coreMv} mV` });
  if (gt) items.push({ label: "核显电压偏移", from: `${current.gtMv} mV`, to: `${draft.gtMv} mV` });
  return { current, coreReady, gtReady, pl, core, gt, items };
}

async function applyDraft(profile: PowerProfile, target: PowerTarget, draft: PowerDraft) {
  const dirty = powerChanges(profile, target, draft);
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
  const load = useCallback(async (reset: PowerTarget | "all" | null) => {
    const id = ++loadId.current;
    let next: PowerProfile;
    try { next = await readPowerProfile(); }
    catch (error) { if (id === loadId.current) setProfile(null); throw error; }
    if (id !== loadId.current) return;
    const before = previous.current;
    setDrafts((current) => Object.fromEntries((["runtime", "saved"] as const).map((key) => {
      const draft = current[key];
      const dirty = before && draft && powerChanges(before, key, draft).items.length > 0;
      return [key, reset !== "all" && reset !== key && dirty ? draft : draftOf(next, key)];
    })) as Record<PowerTarget, PowerDraft>);
    if (!before && !next.msrRuntimeAvailable) setTarget("saved");
    previous.current = next; setProfile(next); setError(null);
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void load(null).catch((cause) => { if (!disposed) setError(String(cause)); });
    return () => { disposed = true; ++loadId.current; };
  }, [active, revision, load]);
  const run = async (action: () => Promise<void>, success: string, reset: PowerTarget | "all") => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await action(); await load(reset); toast({ kind: "success", title: success }); }
    catch (cause) {
      const message = String(cause);
      try { await load(null); setError(message); }
      catch (refreshError) { setError(`${message}；回读失败：${String(refreshError)}`); }
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const draft = drafts[target];
  const dirty = profile && draft ? powerChanges(profile, target, draft) : null;
  const change = (patch: Partial<PowerDraft>) => setDrafts((current) => {
    const value = current[target];
    if (!value || running.current) return current;
    const next = { ...value, ...patch };
    if (patch.pl1 != null) next.pl2 = Math.max(next.pl1, next.pl2);
    if (patch.pl2 != null) next.pl1 = Math.min(next.pl1, next.pl2);
    return { ...current, [target]: next };
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
  return { profile, draft, target, setTarget, error, busy, dirty, change, reviews, restore,
    retry: () => { void load(null).catch((cause) => setError(String(cause))); } };
}
