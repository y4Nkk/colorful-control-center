import { useEffect, useState } from "react";
import { previewLanPolicy, type LanChange, type LanRequest } from "../../api/client";

export function useLanWakePreview(active: boolean, request: LanRequest | null) {
  const revision = request?.expectedRevision, deviceId = request?.deviceId;
  const [result, setResult] = useState<{ revision: string; deviceId: string; changes: LanChange[]; error: string | null } | null>(null);
  useEffect(() => {
    if (!active || !revision || !deviceId) { setResult(null); return; }
    let disposed = false;
    void previewLanPolicy({ expectedRevision: revision, deviceId, powerSaving: null, applyWakePatch: true })
      .then(changes => { if (!disposed) setResult({ revision, deviceId, changes, error: null }); })
      .catch(error => { if (!disposed) setResult({ revision, deviceId, changes: [], error: String(error) }); });
    return () => { disposed = true; };
  }, [active, revision, deviceId]);
  const current = active && result?.revision === revision && result?.deviceId === deviceId ? result : null;
  return { changes: current?.changes ?? [], error: current?.error ?? null, ready: current != null && !current.error };
}
