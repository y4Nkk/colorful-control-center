import { useEffect, useRef, useState } from "react";
import { readSystemOverview, type SystemOverview } from "../api/client";

const POLL_INTERVAL_MS = 5_000;
export const MEMORY_HISTORY_MS = 5 * 60_000;
export const MEMORY_MAX_GAP_MS = 2 * POLL_INTERVAL_MS;

export interface MemorySample {
  atMs: number;
  capacity: SystemOverview["memory"];
}

interface OverviewState {
  overview: SystemOverview | null;
  memoryHistory: MemorySample[];
}

export function useSystemOverview(active: boolean) {
  const [state, setState] = useState<OverviewState>({ overview: null, memoryHistory: [] });
  const inFlight = useRef<Promise<SystemOverview> | null>(null);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const record = (overview: SystemOverview | null) => {
      if (disposed) return;
      const atMs = Date.now();
      setState(({ memoryHistory }) => ({
        overview,
        memoryHistory: [
          ...memoryHistory.filter(sample => sample.atMs >= atMs - MEMORY_HISTORY_MS && sample.atMs < atMs),
          { atMs, capacity: overview?.memory ?? null },
        ],
      }));
    };
    const refresh = async () => {
      // IPC 读取不能取消；重新进入页面须等待旧请求结束。
      if (inFlight.current) await inFlight.current.catch(() => undefined);
      if (disposed) return;
      const request = readSystemOverview();
      inFlight.current = request;
      try {
        record(await request);
      } catch (cause) {
        if (!disposed) console.warn("系统概览读取失败", cause);
        record(null);
      } finally {
        if (inFlight.current === request) inFlight.current = null;
        if (!disposed) timer = setTimeout(() => void refresh(), POLL_INTERVAL_MS);
      }
    };
    void refresh();
    return () => { disposed = true; clearTimeout(timer); };
  }, [active]);
  return state;
}
