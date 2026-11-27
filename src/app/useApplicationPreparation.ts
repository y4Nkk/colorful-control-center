import { useEffect, useState } from "react";
import { readApplicationPreparation, toCommandError, type ApplicationPreparation } from "../api/client";
import { applyAppearance } from "../lib/appearance";

export function useApplicationPreparation() {
  const [snapshot, setSnapshot] = useState<ApplicationPreparation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let disposed = false;
    let timer: number | undefined;
    let appliedAppearance = false;
    async function check() {
      try {
        const next = await readApplicationPreparation();
        if (disposed) return;
        if (!appliedAppearance && next.appearance) {
          applyAppearance(next.appearance);
          appliedAppearance = true;
        }
        setSnapshot(current => current?.stage === next.stage && next.stage !== "failed" ? current : next);
        if (next.stage !== "ready" && next.stage !== "failed") {
          timer = window.setTimeout(() => void check(), 200);
        }
      } catch (failure) {
        if (!disposed) setError(toCommandError(failure).message);
      }
    }
    void check();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [attempt]);

  return {
    snapshot, error,
    recheck: () => { setError(null); setAttempt(value => value + 1); },
  };
}
