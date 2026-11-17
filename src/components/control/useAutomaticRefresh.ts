import { useCallback, useEffect, useRef, useState } from "react";
import { enableAutomaticRefresh, readAutomaticRefresh, restoreAutomaticRefresh, type AutomaticRefreshStatus } from "../../api/client";
import { useAutomaticDisplayTrialId } from "../../app/automatic-display-trial";

export function useAutomaticRefresh(active: boolean, revision: number) {
  const [view, setView] = useState<AutomaticRefreshStatus | null>(null);
  const trialId = useAutomaticDisplayTrialId();
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const writing = useRef(false), generation = useRef(0);
  const reload = useCallback(async (afterWrite = false) => {
    if (writing.current && !afterWrite) return;
    const id = ++generation.current;
    try {
      const next = await readAutomaticRefresh();
      if (id === generation.current) { setView(next); setError(null); }
    } catch (error) {
      if (id === generation.current) { setView(null); setError(String(error)); }
    }
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload();
    const timer = setInterval(() => { void reload(); }, 1000);
    return () => { clearInterval(timer); ++generation.current; };
  }, [active, revision, reload, trialId]);
  const run = async (action: (revision: string) => Promise<void>) => {
    if (!view || writing.current) return;
    writing.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(view.revision); await reload(true); }
    catch (error) { await reload(true); setError(String(error)); }
    finally { writing.current = false; setBusy(false); }
  };
  return { view, busy, error, retry: () => reload(), enable: () => run(enableAutomaticRefresh), restore: () => run(restoreAutomaticRefresh) };
}
