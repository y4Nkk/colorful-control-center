import { useCallback, useEffect, useRef, useState } from "react";
import { ENERGY_POLICY_LABEL, readEnergyPolicy, type EnergyPolicy, type EnergyPolicyProfile } from "../../api/client";
import { toast } from "../use-toast";
import { ENERGY_FLOORS, ENERGY_CEILINGS } from "../../lib/energy-policy";

export function useEnergyPolicy(active: boolean, revision: number, onChanged: () => void, onBusy: (busy: boolean) => void) {
  const [profile, setProfile] = useState<EnergyPolicyProfile | null>(null);
  const [draft, setDraft] = useState<EnergyPolicy | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const accepted = useRef<EnergyPolicy | null>(null);
  const manualLimits = useRef<EnergyPolicy["limits"] | null>(null);
  const generation = useRef(0), writing = useRef(false), reading = useRef<Promise<EnergyPolicyProfile | null> | null>(null);
  const reload = useCallback(async (reset: boolean, afterWrite = false) => {
    if (reading.current || writing.current && !afterWrite) return;
    const id = ++generation.current, pending = readEnergyPolicy(); reading.current = pending;
    let next: EnergyPolicyProfile | null;
    try { next = await pending; } catch (error) {
      if (id === generation.current) { setProfile(null); throw error; }
      return;
    }
    finally { if (reading.current === pending) reading.current = null; }
    if (id !== generation.current) return;
    setLoaded(true);
    if (!next) { setProfile(null); return; }
    const initial: EnergyPolicy = next.saved ?? { enabled: false, everyDay: true, weekdays: 0, slots: [null, null], automatic: true, limits: next.capability.factory };
    const before = accepted.current; accepted.current = structuredClone(initial);
    setProfile(next);
    setDraft(previous => reset || !previous || JSON.stringify(previous) === JSON.stringify(before)
      ? structuredClone(initial) : { ...previous, enabled: initial.enabled });
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload(false).catch(e => setError(String(e)));
    return () => { ++generation.current; reading.current = null; };
  }, [active, revision, reload]);
  const hasProfile = !!profile;
  useEffect(() => {
    if (!active || !hasProfile) return;
    const timer = setInterval(() => { void reload(false).catch(e => setError(String(e))); }, 5000);
    return () => clearInterval(timer);
  }, [active, revision, hasProfile, reload]);
  const run = async (action: () => Promise<void>, { changed = true, preserveDraft = false }: { changed?: boolean; preserveDraft?: boolean } = {}) => {
    if (!active || writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); onBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      await action(); await reload(!preserveDraft, true);
      if (changed) toast({ kind: "success", title: `${ENERGY_POLICY_LABEL}已发送并保存` });
    } catch (e) {
      setError(String(e)); writing.current = false;
      await reload(false).catch(e => setError(previous => `${previous}；重新读取：${e}`));
    } finally { writing.current = false; setBusy(false); onBusy(false); if (changed) onChanged(); }
  };
  const dirty = !!draft && JSON.stringify(draft) !== JSON.stringify(accepted.current);
  const changeAutomatic = (automatic: boolean) => {
    if (!draft || !profile) return;
    if (automatic) {
      manualLimits.current = draft.limits;
      setDraft({ ...draft, automatic, limits: profile.capability.factory });
    } else {
      const limits = manualLimits.current ?? draft.limits;
      setDraft({ ...draft, automatic, limits: ENERGY_FLOORS.includes(limits.floor) && ENERGY_CEILINGS.includes(limits.ceiling)
        ? limits : { floor: 40, ceiling: 100 } });
    }
  };
  return { profile, current: accepted.current, draft, setDraft, dirty, changeAutomatic, busy, error, loaded, run,
    reread: () => run(async () => {}, { changed: false, preserveDraft: true }) };
}
