import { useCallback, useEffect, useRef, useState } from "react";
import { applyOemPowerMode, readOemPowerMode, restoreOemPowerMode,
  type OemPowerMode, type OpResult } from "../../api/client";

export function useOemPowerMode(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<OemPowerMode | null>(null);
  const [draft, setDraft] = useState<number | null>(null);
  const [hasSelection, setHasSelection] = useState(false);
  const selected = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [reading, setReading] = useState(true);
  const loading = useRef(0);
  const running = useRef(false);
  const load = useCallback(async (reset: boolean, operationError: string | null = null) => {
    const id = ++loading.current;
    setReading(true);
    try {
      const next = await readOemPowerMode();
      if (id !== loading.current) return;
      if (reset) { selected.current = false; setHasSelection(false); }
      setDraft((current) => !reset && selected.current ? current : next.currentMode);
      setProfile(next);
      setError(operationError);
    } catch (cause) {
      if (id !== loading.current) return;
      setProfile(null);
      setError([operationError, String(cause)].filter(Boolean).join("；"));
    } finally {
      if (id === loading.current) setReading(false);
    }
  }, []);
  const retry = useCallback(() => { void load(false); }, [load]);
  useEffect(() => {
    if (!active || running.current) return;
    retry();
    // 切页可作废普通读取；写入后的回读仍须完成，避免保活页停留在忙碌状态。
    return () => { if (!running.current) ++loading.current; };
  }, [active, revision, retry]);
  const run = async (action: () => Promise<OpResult>) => {
    if (running.current || reading || !profile) return;
    ++loading.current;
    running.current = true; setBusy(true); setError(null);
    try {
      const result = await action();
      if (!result.ok) throw new Error(result.message);
      await load(true);
    } catch (cause) {
      await load(false, String(cause));
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const target = profile?.targets.find((candidate) => candidate.mode === draft);
  return { profile, draft, hasSelection, select: (mode: number) => { selected.current = true; setHasSelection(true); setDraft(mode); }, error, busy, reading, target, retry,
    apply: () => target ? run(() => applyOemPowerMode(target)) : Promise.resolve(),
    restore: (revision: string) => run(() => restoreOemPowerMode(revision)) };
}
