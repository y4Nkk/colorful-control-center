import { useEffect, useRef, useState } from "react";
import { getHardwareSnapshot, onHardwareSnapshot, type HardwareSnapshot } from "../api/client";

const HISTORY_MS = 5 * 60 * 1000;
export function useHardwareHistory(active: boolean) {
  const [live, setLive] = useState<HardwareSnapshot | null>(null);
  const [samples, setSamples] = useState<HardwareSnapshot[]>([]);
  const history = useRef<HardwareSnapshot[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    let off: (() => void) | undefined;
    const fail = (cause: unknown) => { if (!disposed) setError(String(cause)); };
    const accept = (snapshot: HardwareSnapshot) => {
      if (disposed || snapshot.sampledAtMs <= (history.current[history.current.length - 1]?.sampledAtMs ?? 0)) return;
      setLive(snapshot); setError(null);
      history.current = [...history.current, snapshot]
        .filter((item) => item.sampledAtMs >= snapshot.sampledAtMs - HISTORY_MS).slice(-301);
      setSamples(history.current);
    };
    void onHardwareSnapshot(accept).then((unlisten) => { if (disposed) unlisten(); else off = unlisten; }).catch(fail);
    void getHardwareSnapshot().then(accept).catch(fail);
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => { disposed = true; off?.(); clearInterval(timer); };
  }, [active, retry]);
  return { live, samples, error, stale: live != null && now - live.sampledAtMs > 3000,
    retry: () => setRetry((tick) => tick + 1) };
}
