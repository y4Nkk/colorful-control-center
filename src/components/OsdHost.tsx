import { useEffect, useRef, useState } from "react";
import { onOsd, readOsd, type OsdSnapshot } from "../api/client";
import { applyAppearance, observeSystemTheme } from "../lib/appearance";
import { Surface } from "./Surface";

export function OsdHost() {
  const [snapshot, setSnapshot] = useState<OsdSnapshot | null>(null);
  const newest = useRef(-1);
  useEffect(() => {
    document.documentElement.dataset.surface = "osd";
    const stopTheme = observeSystemTheme();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const accept = (next: OsdSnapshot) => {
      if (disposed || next.revision < newest.current) return;
      newest.current = next.revision;
      if (next.appearance) applyAppearance(next.appearance);
      setSnapshot(next);
    };
    void onOsd(accept).then(stop => {
      if (disposed) { stop(); return; }
      unlisten = stop;
      return readOsd().then(accept);
    }).catch(error => console.error("屏显读取失败", error));
    return () => { disposed = true; unlisten?.(); stopTheme(); };
  }, []);
  const message = snapshot?.message;
  if (!message || !snapshot.visible) return null;
  const level = message.level;
  const ratio = level && level.max > 0 ? Math.min(1, Math.max(0, level.value / level.max)) : 0;
  return <div className="ccc-osd" role="status" aria-live="polite">
    <Surface as="section" variant="card" className={`ccc-osd-card${message.error ? " is-error" : ""}`}>
      <div className="ccc-osd-content"><strong>{message.title}</strong><p>{message.detail}</p></div>
      {level && <div className="ccc-osd-level" aria-label={`${level.value} / ${level.max}`}>
        <svg viewBox="0 0 48 48" aria-hidden="true"><circle className="ccc-osd-track" cx="24" cy="24" r="20" />
          <circle cx="24" cy="24" r="20" pathLength="1" strokeDasharray={`${ratio} 1`} transform="rotate(-90 24 24)" /></svg>
        <span>{level.value}<small> / {level.max}</small></span>
      </div>}
    </Surface>
  </div>;
}
