import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { readMemoryCapacity, readSystemInventory, type HardwareCapacity, type SystemInventory } from "../api/client";

const MEMORY_INTERVAL_MS = 5_000;
const INVENTORY_INTERVAL_MS = 30_000;
export const MEMORY_HISTORY_MS = 5 * 60_000;
export const MEMORY_MAX_GAP_MS = 2 * MEMORY_INTERVAL_MS;

export interface MemorySample {
  atMs: number;
  capacity: HardwareCapacity | null;
}

export interface SystemOverview {
  inventory: SystemInventory | null;
  memory: HardwareCapacity | null;
}

function subscribeToVisibility(notify: () => void) {
  document.addEventListener("visibilitychange", notify);
  return () => document.removeEventListener("visibilitychange", notify);
}
const getVisibility = () => document.visibilityState === "visible";

function useOverviewPolling<T>(active: boolean, interval: number, read: (first: boolean) => Promise<T>,
  record: (value: T | null) => void) {
  const inFlight = useRef<Promise<T> | null>(null);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refresh = async (first: boolean) => {
      // IPC 读取不能取消；重新进入页面须等待旧请求结束。
      if (inFlight.current) await inFlight.current.catch(() => undefined);
      if (disposed) return;
      const request = read(first);
      inFlight.current = request;
      try {
        const value = await request;
        if (!disposed) record(value);
      } catch (cause) {
        if (!disposed) {
          console.warn("系统概览读取失败", cause);
          record(null);
        }
      } finally {
        if (inFlight.current === request) inFlight.current = null;
        if (!disposed) timer = setTimeout(() => void refresh(false), interval);
      }
    };
    void refresh(true);
    return () => { disposed = true; clearTimeout(timer); };
  }, [active, interval, read, record]);
}

export function useSystemOverview(active: boolean) {
  const visible = useSyncExternalStore(subscribeToVisibility, getVisibility);
  const [inventory, setInventory] = useState<SystemInventory | null>(null);
  const [memoryHistory, setMemoryHistory] = useState<MemorySample[]>([]);
  const recordMemory = useCallback((capacity: HardwareCapacity | null) => {
    const atMs = Date.now();
    setMemoryHistory(history => [
      ...history.filter(sample => sample.atMs >= atMs - MEMORY_HISTORY_MS && sample.atMs < atMs),
      { atMs, capacity },
    ]);
  }, []);
  useOverviewPolling(active && visible, MEMORY_INTERVAL_MS, readMemoryCapacity, recordMemory);
  useOverviewPolling(active && visible, INVENTORY_INTERVAL_MS, readSystemInventory, setInventory);
  const overview: SystemOverview = { inventory, memory: memoryHistory[memoryHistory.length - 1]?.capacity ?? null };
  return { overview, memoryHistory };
}
