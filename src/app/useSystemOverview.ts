import { useEffect, useRef, useState } from "react";
import { readSystemOverview, type SystemOverview } from "../api/client";

export function useSystemOverview(active: boolean) {
  const [overview, setOverview] = useState<SystemOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [reading, setReading] = useState(false);
  const inFlight = useRef<Promise<SystemOverview> | null>(null);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refresh = async () => {
      setReading(true);
      // IPC 读取不能取消；重试或重新进入页面须等待旧请求结束。
      if (inFlight.current) await inFlight.current.catch(() => undefined);
      if (disposed) return;
      const request = readSystemOverview();
      inFlight.current = request;
      try {
        const next = await request;
        if (!disposed) { setOverview(next); setError(null); }
      } catch (cause) {
        if (!disposed) { setOverview(null); setError(String(cause)); }
      } finally {
        if (inFlight.current === request) inFlight.current = null;
        if (!disposed) {
          setReading(false);
          timer = setTimeout(() => void refresh(), 5000);
        }
      }
    };
    void refresh();
    return () => { disposed = true; clearTimeout(timer); };
  }, [active, attempt]);
  return { overview, error, reading: active && reading, retry: () => setAttempt(previous => previous + 1) };
}
