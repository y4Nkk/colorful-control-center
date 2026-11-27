import { useCallback, useEffect, useSyncExternalStore } from "react";
import { onHardwareSnapshot, updateHardwareMonitoring, toCommandError, type HardwareSnapshot } from "../api/client";

export const HARDWARE_HISTORY_MS = 5 * 60_000;
export const HARDWARE_HISTORY_GAP_MS = 5_000;
export const CPU_READING_MAX_AGE_MS = 2_500;
const STALE_MS = 3_000;

export function freshCpuReading(sample: HardwareSnapshot | null) {
  const reading = sample?.cpuUtility;
  if (!sample || !reading) return null;
  const age = sample.sampledAtMs - reading.sampledAtMs;
  return age >= 0 && age <= CPU_READING_MAX_AGE_MS ? reading : null;
}

interface History {
  live: HardwareSnapshot | null;
  samples: readonly HardwareSnapshot[];
}
const emptyHistory: History = { live: null, samples: [] };
const emptyStatus = { connected: false, mode: null as number | null, fanOffline: false, gpuPaused: false,
  stale: false, error: null as string | null };
let history = emptyHistory;
let status = emptyStatus;
let previousArrival: number | undefined;
let request = Promise.resolve();
const listeners = new Set<() => void>();
let reconnect: (() => void) | undefined;

function publish() {
  for (const listener of listeners) listener();
}

function updateStatus(changes: Partial<typeof status>) {
  const changed = Object.keys(changes) as (keyof typeof status)[];
  if (changed.some(key => status[key] !== changes[key])) status = { ...status, ...changes };
}

function accept(live: HardwareSnapshot) {
  const previous = history.live?.sampledAtMs;
  const samples = previous !== undefined && live.sampledAtMs < previous ? [] : history.samples;
  history = { live, samples: [...samples.filter(sample => sample.sampledAtMs < live.sampledAtMs &&
    sample.sampledAtMs >= live.sampledAtMs - HARDWARE_HISTORY_MS), live].slice(-301) };
  previousArrival = performance.now();
  updateStatus({ connected: true, mode: live.oemPowerMode, fanOffline: !live.dchuAvailable,
    gpuPaused: live.nvidiaSuspended, stale: false, error: null });
  publish();
}

function setDemand(active: boolean) {
  const next = request.then(() => updateHardwareMonitoring(active));
  request = next.catch(() => undefined);
  return next;
}

function connect() {
  let disposed = false, requested = false, off: (() => void) | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const fail = (error: unknown) => {
    if (disposed) return;
    clearTimeout(timer);
    updateStatus({ error: toCommandError(error).message });
    publish();
  };
  const expire = () => {
    if (disposed) return;
    if (history.live) {
      updateStatus({ stale: true });
      publish();
    } else fail("硬件采样未响应");
  };
  const arm = () => {
    clearTimeout(timer);
    const age = previousArrival === undefined ? 0 : performance.now() - previousArrival;
    if (age >= STALE_MS) expire();
    else timer = setTimeout(expire, STALE_MS - age);
  };
  void onHardwareSnapshot(snapshot => {
    if (disposed) return;
    accept(snapshot);
    arm();
  }).then(stop => {
    if (disposed) { stop(); return; }
    off = stop;
    requested = true;
    return setDemand(true);
  }).catch(fail);
  arm();
  return () => {
    disposed = true;
    off?.();
    clearTimeout(timer);
    if (requested) void setDemand(false).catch(error => console.error("停止硬件监控失败", error));
  };
}

export function useHardwareMonitoring(active: boolean) {
  useEffect(() => {
    if (!active) return;
    let stop: (() => void) | undefined;
    const syncVisibility = () => {
      if (document.visibilityState !== "visible") { stop?.(); stop = undefined; }
      else stop ??= connect();
    };
    reconnect = () => { stop?.(); stop = undefined; syncVisibility(); };
    document.addEventListener("visibilitychange", syncVisibility);
    syncVisibility();
    return () => {
      document.removeEventListener("visibilitychange", syncVisibility);
      reconnect = undefined;
      stop?.();
    };
  }, [active]);
}

function useMonitoring<T>(active: boolean, snapshot: () => T, empty: T) {
  const subscribe = useCallback((listener: () => void) => {
    if (active) listeners.add(listener);
    return () => { listeners.delete(listener); };
  }, [active]);
  return useSyncExternalStore(subscribe, () => active ? snapshot() : empty);
}

export const useHardwareHistory = (active: boolean) => useMonitoring(active, () => history, emptyHistory);
export const useHardwareStatus = (active: boolean) => useMonitoring(active, () => status, emptyStatus);
export const retryHardwareMonitoring = () => reconnect?.();
