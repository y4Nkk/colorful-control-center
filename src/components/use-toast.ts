import { useSyncExternalStore } from "react";

export type ToastKind = "info" | "success" | "warning" | "error";

interface ToastOptions {
  title: string;
  description?: string;
  kind: ToastKind;
  /** 0 表示保留到手动关闭。 */
  durationMs?: number;
}

interface Toast extends ToastOptions { id: number }
interface Timer {
  remaining: number;
  running?: { handle: ReturnType<typeof setTimeout>; deadline: number };
}

const LIMIT = 2;
const DURATION: Record<ToastKind, number> = { info: 4500, success: 3200, warning: 6000, error: 10000 };
const listeners = new Set<() => void>();
const timers = new Map<number, Timer>();
let count = 0;
let toasts: readonly Toast[] = [];

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

function publish(next: readonly Toast[]) {
  const retained = new Set(next.map(item => item.id));
  for (const [id, timer] of timers) {
    if (retained.has(id)) continue;
    clearTimeout(timer.running?.handle);
    timers.delete(id);
  }
  toasts = next;
  listeners.forEach(listener => listener());
}

function dismiss(id: number) {
  if (toasts.some(item => item.id === id)) publish(toasts.filter(item => item.id !== id));
}

export function toast(options: ToastOptions) {
  if (!options.title.trim()) throw new TypeError("Toast requires a title");
  const remaining = options.durationMs ?? DURATION[options.kind];
  if (!Number.isFinite(remaining) || remaining < 0) throw new RangeError("Invalid toast duration");
  const id = ++count;
  const byPriority = (left: Toast, right: Toast) => Number(right.kind === "error") - Number(left.kind === "error") || right.id - left.id;
  const next = [{ ...options, id }, ...toasts]
    .sort((left, right) => Number(right.durationMs === 0) - Number(left.durationMs === 0) || byPriority(left, right))
    .slice(0, LIMIT).sort(byPriority);
  if (remaining > 0) timers.set(id, { remaining });
  publish(next);
  return () => dismiss(id);
}

function pause(id: number) {
  const timer = timers.get(id);
  if (!timer?.running) return;
  clearTimeout(timer.running.handle);
  timer.remaining = Math.max(0, timer.running.deadline - performance.now());
  timer.running = undefined;
}

function resume(id: number) {
  const timer = timers.get(id);
  if (!timer || timer.running) return;
  timer.running = {
    handle: setTimeout(() => dismiss(id), timer.remaining),
    deadline: performance.now() + timer.remaining,
  };
}

const getSnapshot = () => toasts;
const clear = () => publish([]);

export function useToast() {
  return { toasts: useSyncExternalStore(subscribe, getSnapshot), dismiss, clear, pause, resume };
}
