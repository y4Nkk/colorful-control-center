import { useCallback, useEffect, useRef, useState } from "react";
import { readKeymapRecording, startKeymapRecording, stopKeymapRecording, type KeymapRecording } from "../../api/client";

export function useKeymapRecording(active: boolean) {
  const [capture, setCapture] = useState<KeymapRecording | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const writing = useRef(false), recording = useRef(false), reading = useRef(false), generation = useRef(0);
  const accept = (next: KeymapRecording | null) => { recording.current = next?.active ?? false; setCapture(next); };
  const stop = useCallback(async () => {
    ++generation.current;
    try { const next = await stopKeymapRecording(); accept(next); }
    catch (error) { setError(String(error)); }
  }, []);
  useEffect(() => {
    if (!active) { ++generation.current; if (recording.current || writing.current) void stop(); return; }
    const timer = setInterval(() => {
      if (!recording.current || reading.current || writing.current) return;
      const id = generation.current; reading.current = true;
      void readKeymapRecording().then(next => { if (id === generation.current) accept(next); })
        .catch(error => { setError(String(error)); void stop(); }).finally(() => { reading.current = false; });
    }, 250);
    return () => { clearInterval(timer); ++generation.current; if (recording.current || writing.current) void stop(); };
  }, [active, stop]);
  const start = async () => {
    if (writing.current || recording.current) return;
    writing.current = true; setBusy(true); setError(null); const id = ++generation.current;
    try { const next = await startKeymapRecording(); if (id === generation.current) accept(next); else await stopKeymapRecording(); }
    catch (error) { setError(String(error)); }
    finally { writing.current = false; setBusy(false); }
  };
  return { capture, busy, error, start, stop };
}
