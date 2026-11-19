import { useCallback, useEffect, useRef, useState } from "react";
import { pausePowerProfilePolicy, readPowerProfilePolicy, reloadPowerProfilePolicy, resetPowerProfilePolicy, restorePowerProfilePolicy, savePowerProfilePolicy, setPowerProfilePolicyEnabled,
  type PowerPolicyRecord, type PowerPolicyRequest, type PowerPolicyTrigger, type PowerPolicyView } from "../../api/client";
import { loadBackgroundProfile, useProfileList, type ProfileSelection } from "../../app/useProfileReads";
import { toast } from "../use-toast";

export const POWER_RULES: { key: PowerPolicyTrigger; label: string }[] = [{ key: "ac", label: "插电时" }, { key: "battery", label: "使用电池时" }, { key: "resume", label: "唤醒后" }];
interface Draft { ac: string; battery: string; resume: string }
interface Choice { selection: ProfileSelection | null; error: string | null }
const EMPTY: Record<PowerPolicyTrigger, Choice> = { ac: { selection: null, error: null }, battery: { selection: null, error: null }, resume: { selection: null, error: null } };
function toDraft(record: PowerPolicyRecord | null): Draft { return { ac: record?.ac?.profile ?? "", battery: record?.battery?.profile ?? "", resume: record?.resume?.profile ?? "" }; }

export function usePowerProfilePolicy(active: boolean, revision: number) {
  const [view, setView] = useState<PowerPolicyView | null>(null), [draft, updateDraft] = useState<Draft>(toDraft(null));
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [readError, setReadError] = useState<string | null>(null), [listError, setListError] = useState<string | null>(null);
  const [selectionRevision, setSelectionRevision] = useState(0);
  const generation = useRef(0), dirty = useRef(false), writing = useRef(false), reading = useRef<Promise<PowerPolicyView> | null>(null);
  const { profiles, reload: reloadList, status: profileStatus } = useProfileList(active, setListError);
  const choices = useSelections(active, draft, revision + selectionRevision);
  const reload = useCallback(async (reset = false, afterWrite = false) => {
    if (writing.current && !afterWrite) return;
    if (reading.current) { if (!afterWrite) return; await reading.current.catch(() => {}); }
    const id = ++generation.current, pending = readPowerProfilePolicy(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      setView(next);
      setReadError(null);
      if (reset || !dirty.current) { dirty.current = false; updateDraft(toDraft(next.record)); }
    } catch (error) { if (id === generation.current) { setView(null); setReadError(String(error)); } throw error; }
    finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload().catch(() => {});
    const timer = setInterval(() => { void reload().catch(() => {}); }, 1_000);
    return () => { ++generation.current; clearInterval(timer); };
  }, [active, revision, reload]);
  const setDraft = (next: Draft) => { dirty.current = true; updateDraft(next); };
  const request = makeRequest(view?.record ?? null, draft, choices);
  const staleRules = profileStatus === "ready" ? POWER_RULES.filter(({ key }) => {
    const binding = view?.record?.[key];
    return binding && profiles.find(profile => profile.name === binding.profile)?.revision !== binding.digest;
  }) : [];
  const run = async (action: () => Promise<void>, title: string, reset = true) => {
    if (!active || writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await action(); await reload(reset, true);
      if (reset) setSelectionRevision(value => value + 1);
      toast({ kind: "success", title });
    } catch (error) { setError(String(error)); await reload(false, true).catch(() => {}); }
    finally { writing.current = false; setBusy(false); }
  };
  const pause = () => run(async () => {
    await pausePowerProfilePolicy();
    setView(previous => previous && { ...previous, paused: true, active: false, pending: null });
  }, "自动应用已暂停", false);
  const toggle = (enabled: boolean) => view?.record
    ? run(() => setPowerProfilePolicyEnabled(view.record!, enabled), enabled ? "自动档案已启用" : "自动档案已停用", false)
    : Promise.resolve();
  return { view, draft, setDraft, profiles, choices, busy, edited: dirty.current, staleRules, error: error ?? readError ?? listError, request,
    save: () => request ? run(() => savePowerProfilePolicy(request), "自动档案规则已保存") : Promise.resolve(),
    pause, toggle, restore: () => view?.record ? run(() => restorePowerProfilePolicy(view.record!), "初始规则已恢复，自动应用已暂停") : Promise.resolve(),
    reset: () => view?.resetRevision ? run(() => resetPowerProfilePolicy(view.resetRevision!), "旧规则已清除，自动应用保持停用") : Promise.resolve(),
    reread: () => run(async () => { await reloadPowerProfilePolicy(); await reloadList(); setListError(null); }, "规则已重新读取，自动应用已暂停") };
}

function useSelections(active: boolean, draft: Draft, revision: number) {
  const [choices, setChoices] = useState<Record<PowerPolicyTrigger, Choice>>(EMPTY);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    setChoices(EMPTY);
    void Promise.allSettled(POWER_RULES.map(rule => draft[rule.key] ? loadBackgroundProfile(draft[rule.key]) : Promise.resolve(null))).then(results => {
      if (disposed) return;
      const next = { ...EMPTY };
      results.forEach((result, index) => { next[POWER_RULES[index].key] = result.status === "fulfilled"
        ? { selection: result.value, error: null } : { selection: null, error: String(result.reason) }; });
      setChoices(next);
    });
    return () => { disposed = true; };
  }, [active, draft.ac, draft.battery, draft.resume, revision]);
  return choices;
}

function makeRequest(expected: PowerPolicyRecord | null, draft: Draft, choices: Record<PowerPolicyTrigger, Choice>): PowerPolicyRequest | null {
  if (!expected) return null;
  const selected = (key: PowerPolicyTrigger) => {
    const choice = choices[key].selection;
    return choice && draft[key] === choice.file.name && ![choice.modeError, choice.displayError, choice.gpuPowerError, choice.graphicsModeError].some(Boolean) ? choice : null;
  };
  if (POWER_RULES.some(rule => draft[rule.key] && !selected(rule.key))) return null;
  const target = (key: PowerPolicyTrigger) => { const choice = selected(key); return draft[key] && choice ? { file: choice.file, mode: choice.modeAuthorization } : null; };
  return { expected, enabled: expected.enabled && POWER_RULES.some(rule => draft[rule.key]), ac: target("ac"), battery: target("battery"), resume: target("resume") };
}
