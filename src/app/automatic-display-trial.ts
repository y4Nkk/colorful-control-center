import { useSyncExternalStore } from "react";
import { cancelDisplayTrial, keepDisplayMode, readAutomaticDisplayTrial, type DisplayPending } from "../api/client";

interface State { pending: DisplayPending | null; busy: boolean; error: string | null; readError: string | null }
let state: State = { pending: null, busy: false, error: null, readError: null };
let generation = 0;
const listeners = new Set<() => void>();
function update(next: State) { state = next; listeners.forEach(listener => listener()); }
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
export function useAutomaticDisplayTrial() {
  return useSyncExternalStore(subscribe, () => state);
}
export const useAutomaticDisplayTrialId = () => useSyncExternalStore(subscribe, () => state.pending?.trialId ?? null);
export async function refreshAutomaticDisplayTrial(clearActionError = false) {
  if (state.busy) return Boolean(state.pending);
  if (clearActionError) update({ ...state, error: null });
  const id = ++generation;
  try {
    const pending = await readAutomaticDisplayTrial();
    if (id !== generation) return Boolean(state.pending);
    update({ ...state, pending, readError: null, error: pending?.trialId === state.pending?.trialId ? state.error : null });
  } catch (error) {
    if (id === generation) update({ ...state, readError: String(error) });
  }
  return Boolean(state.pending);
}
export async function resolveAutomaticDisplayTrial(keep: boolean) {
  const pending = state.pending;
  if (!pending || pending.trialId == null || state.busy) return;
  const target = pending.targets[0];
  if (keep && (!target || state.readError)) return;
  ++generation; update({ ...state, busy: true, error: null });
  try {
    if (keep) await keepDisplayMode(pending.trialId, target);
    else await cancelDisplayTrial(pending.trialId);
    update({ ...state, busy: false });
    await refreshAutomaticDisplayTrial();
  } catch (error) {
    const message = String(error);
    update({ ...state, busy: false, error: message });
    await refreshAutomaticDisplayTrial();
    return message;
  }
}
