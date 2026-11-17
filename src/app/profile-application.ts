import { useSyncExternalStore } from "react";
import { readProfileApplication, type ProfileApplication } from "../api/client";

interface State { report: ProfileApplication | null; busy: boolean; error: { kind: "read" | "action"; message: string } | null }
let state: State = { report: null, busy: false, error: null };
let generation = 0;
const listeners = new Set<() => void>();
function publish(next: Partial<State>) {
  state = { ...state, ...next };
  listeners.forEach(listener => listener());
}
export function useProfileApplicationState() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => { listeners.delete(listener); }; }, () => state);
}
export async function refreshProfileApplication(clearActionError = false) {
  if (state.busy) return;
  if (clearActionError) publish({ error: null });
  const id = ++generation;
  try {
    const report = await readProfileApplication();
    if (id === generation && !state.busy) publish({ report, error: state.error?.kind === "action" ? state.error : null });
  } catch (error) { if (id === generation && !state.busy && state.error?.kind !== "action") publish({ error: { kind: "read", message: String(error) } }); }
}
export async function runProfileApplication(action: () => Promise<ProfileApplication>) {
  if (state.busy) throw new Error("档案应用正在执行");
  ++generation;
  publish({ busy: true, error: null });
  try {
    const report = await action();
    publish({ report });
    return report;
  } catch (error) { publish({ error: { kind: "action", message: String(error) } }); throw error; }
  finally { publish({ busy: false }); }
}
