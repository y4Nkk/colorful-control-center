import { useCallback, useEffect, useRef, useState } from "react";
import { onHardwareControlsChanged } from "../api/client";
import { useProfileApplicationState } from "./profile-application";
import { useHardwareHistory } from "./useHardwareHistory";

const CONTROL_REFRESH = {
  mode: ["fan", "power", "gpu", "options", "dust"],
  options: ["mode", "fan", "power", "dust"],
  gpu: ["mode"],
  fan: ["mode", "options", "dust"],
  dust: ["mode", "options", "fan"],
} as const;

function useControlRevisions() {
  const [deviceRevision, setDeviceRevision] = useState(0);
  const [revisions, setRevisions] = useState({ fan: 0, power: 0, gpu: 0, mode: 0, options: 0, dust: 0 });
  const application = useProfileApplicationState().report;
  const applicationPhase = application ? `${application.id}:${application.revision}` : null;
  const observedApplication = useRef(applicationPhase);
  const refreshAll = useCallback(() => {
    setDeviceRevision(previous => previous + 1);
    setRevisions(previous => ({ fan: previous.fan + 1, power: previous.power + 1, gpu: previous.gpu + 1,
      mode: previous.mode + 1, options: previous.options + 1, dust: previous.dust + 1 }));
  }, []);
  useEffect(() => {
    if (observedApplication.current === applicationPhase) return;
    observedApplication.current = applicationPhase;
    if (applicationPhase !== null) refreshAll();
  }, [applicationPhase, refreshAll]);
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    void onHardwareControlsChanged(() => {
      if (disposed) return;
      refreshAll();
    }).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(error => console.error("控制状态刷新失败", error));
    return () => { disposed = true; unlisten?.(); };
  }, [refreshAll]);
  const refreshSystemPolicy = useCallback(() => setRevisions(previous => ({ ...previous, mode: previous.mode + 1, options: previous.options + 1 })), []);
  const refreshAfterControl = useCallback((source: keyof typeof CONTROL_REFRESH) => setRevisions(previous => {
    const next = { ...previous };
    for (const domain of CONTROL_REFRESH[source]) next[domain] += 1;
    return next;
  }), []);
  return { deviceRevision, revisions, refreshAll, refreshSystemPolicy, refreshAfterControl, setRevisions };
}

export function useControlModel(monitoring: boolean) {
  const revisions = useControlRevisions();
  const history = useHardwareHistory(monitoring);
  return { ...revisions, history };
}
export type ControlModel = ReturnType<typeof useControlModel>;
export interface HardwareWorkspaceProps { active: boolean; model: ControlModel }
