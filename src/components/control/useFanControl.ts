import { useCallback, useEffect, useRef, useState } from "react";
import { applyFanCurve, applyRpmFanCurves, CUSTOM_FAN_MODE, readFanProfile, restoreFanDefault, restoreFanProfile, setFanMode, setFanOffset,
  type FanId, type FanProfile, type RpmPoint } from "../../api/client";
import { toast } from "../use-toast";
import { normalizeDraft, sameDraft, type CurveDraft } from "./curve";
import { sameRpmCurve } from "./rpmCurve";
import { errText } from "./format";

const checked = async (action: Promise<{ ok: boolean; message: string }>) => {
  const result = await action; if (!result.ok) throw new Error(result.message);
};

export function useFanControl(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<FanProfile | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const loadId = useRef(0);
  const load = useCallback(async () => {
    const id = ++loadId.current;
    let next: FanProfile;
    try { next = await readFanProfile(); }
    catch (cause) { if (id === loadId.current) setReadError(errText(cause)); return false; }
    if (id !== loadId.current) return false;
    setProfile(next); setReadError(null);
    return true;
  }, []);
  useEffect(() => {
    if (!active || running.current) return;
    void load();
    return () => { if (!running.current) ++loadId.current; };
  }, [active, revision, load]);
  const softwareRunning = profile?.software != null;
  useEffect(() => {
    if (!active || !softwareRunning) return;
    const timer = window.setInterval(() => { if (!running.current) void load(); }, 5000);
    return () => window.clearInterval(timer);
  }, [active, softwareRunning, load]);
  const readValid = !!profile && readError === null;
  const run = async (action: () => Promise<{ ok: boolean; message: string }>, recovery = false) => {
    if (!active || running.current || !readValid || !profile || profile.blockedReason) return;
    running.current = true; ++loadId.current; setBusy(true);
    try {
      const current = await readFanProfile();
      if (JSON.stringify(current) !== JSON.stringify(profile)) throw new Error("风扇设置已变化，请重新调整");
      await checked(action());
      const refreshed = await load();
      setActionError(null);
      if (refreshed && recovery) toast({ kind: "success", title: "风扇设置已恢复" });
    } catch (cause) {
      await load();
      setActionError(errText(cause));
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const setMode = (mode: number) => {
    if (!profile || !profile.availableModes.includes(mode) ||
        (mode === CUSTOM_FAN_MODE && (!profile.customSupported || profile.customBlockedReason))) return;
    if (profile.software || mode !== profile.currentMode) void run(() => setFanMode(mode));
  };
  const changeCurve = (id: FanId, value: CurveDraft) => {
    const fan = profile?.fans.find(item => item.id === id && item.present);
    if (!fan || !profile?.customSupported || profile.customBlockedReason || profile.software ||
        sameDraft(normalizeDraft(fan, fan), value)) return;
    return run(() => applyFanCurve(id === "cpu" ? value : null, id === "gpu1" ? value : null, id === "gpu2" ? value : null));
  };
  const changeRpm = (id: FanId, points: RpmPoint[]) => {
    const fan = profile?.rpmFans.find(item => item.id === id);
    if (!fan || !profile?.customSupported || profile.customBlockedReason || profile.software ||
        sameRpmCurve(fan.points, points)) return;
    return run(() => applyRpmFanCurves({ [id]: points }));
  };
  const setOffset = (offset: number) => {
    if (profile?.offsetBlockedReason === null && offset !== profile.offsetPct)
      return run(() => setFanOffset({ mode: profile.currentMode, slot: profile.offsetSlot }, offset));
  };
  const error = [actionError, readError].filter(Boolean).join("；") || null;
  return { profile, error, busy, readValid, setMode, changeCurve, changeRpm, setOffset,
    restore: () => run(restoreFanProfile, true), restoreDefault: () => run(restoreFanDefault, true),
    retry: () => { if (active && !running.current) void load().then(ok => { if (ok) setActionError(null); }); } };
}
