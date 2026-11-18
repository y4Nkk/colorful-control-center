import { createContext, useContext, useId, useLayoutEffect, type ReactNode } from "react";

type Result = boolean | void;
type Commit = () => Result | Promise<Result>;
type Submission = { together: true; onConfirm: () => Promise<boolean> } | { together?: false; onConfirm: Commit };

type ActionOptions = Submission & {
  label: string;
  confirmLabel?: string;
  revision: string;
  disabled?: boolean;
  restore?: boolean;
  children?: ReactNode;
};
export type RegisteredAction = ActionOptions & { id: string };
export type ActionRegistry = ReturnType<typeof createActionRegistry>;
export const ConfigurationActions = createContext<ActionRegistry | null>(null);

export function createActionRegistry() {
  const listeners = new Set<() => void>();
  const entries = new Map<string, RegisteredAction>();
  let snapshot: RegisteredAction[] = [];
  let active = false;
  const publish = () => {
    snapshot = Array.from(entries.values());
    listeners.forEach(listener => listener());
  };
  return {
    setActive(value: boolean) { active = value; },
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
    snapshot: () => snapshot,
    current(action: RegisteredAction) {
      const latest = entries.get(action.id);
      return active && latest?.revision === action.revision ? latest : undefined;
    },
    register(action: RegisteredAction) { entries.set(action.id, action); publish(); },
    remove(id: string) { if (entries.delete(id)) publish(); },
  };
}

/** The module's revision identifies both the displayed preview and its executable target. */
export function ConfigurationAction(options: ActionOptions) {
  const registry = useContext(ConfigurationActions);
  const id = useId();
  useLayoutEffect(() => {
    if (options.disabled) registry?.remove(id);
    else registry?.register({ ...options, id });
  });
  useLayoutEffect(() => () => registry?.remove(id), [registry, id]);
  return null;
}
