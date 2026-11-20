import { useCallback, useEffect, useRef, useState } from "react";
import { applyFanCurve, applyRpmFanCurves, CUSTOM_FAN_MODE, offsetModeAvailable, readFanProfile, restoreFanDefault, restoreFanProfile, setFanMode, setFanOffset,
  type FanId, type FanProfile, type RpmPoint } from "../../api/client";
import { toast } from "../use-toast";
import { normalizeDraft, sameDraft, type CurveDraft } from "./curve";
import { normalizeRpmCurve, sameRpmCurve } from "./rpmCurve";
import { errText } from "./format";

const checked = async (action: Promise<{ ok: boolean; message: string }>) => {
  const result = await action; if (!result.ok) throw new Error(result.message);
};

function writeFirmwareMode(profile: FanProfile, mode: number) {
  if (mode !== CUSTOM_FAN_MODE || profile.curveKind !== "rpmTable") return setFanMode(mode);
  const curves = Object.fromEntries(profile.rpmFans.flatMap(fan => {
    const points = normalizeRpmCurve(fan.points);
    return sameRpmCurve(fan.points, points) ? [] : [[fan.id, points]];
  }));
  return Object.keys(curves).length ? applyRpmFanCurves(curves) : setFanMode(mode);
}

export function useFanControl(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<FanProfile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [readValid, setReadValid] = useState(false);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const loadId = useRef(0);
  const load = useCallback(async () => {
    const id = ++loadId.current;
    let next: FanProfile;
    try { next = await readFanProfile(); }
    catch (cause) { if (id === loadId.current) setReadValid(false); throw cause; }
    if (id !== loadId.current) return;
    setProfile(next); setReadValid(true); setError(null);
  }, []);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    if (!running.current) void load().catch((cause) => { if (!disposed) setError(errText(cause)); });
    return () => { disposed = true; if (!running.current) ++loadId.current; };
  }, [active, revision, load]);
  const softwareRunning = profile?.software != null;
  useEffect(() => {
    if (!active || !softwareRunning) return;
    const timer = window.setInterval(() => { if (!running.current) void load().catch((cause) => setError(errText(cause))); }, 5000);
    return () => window.clearInterval(timer);
  }, [active, softwareRunning, load]);
  const run = async (action: () => Promise<{ ok: boolean; message: string }>, recovery = false) => {
    if (!active || running.current || !readValid || !profile || profile.blockedReason) return;
    running.current = true; ++loadId.current; setBusy(true); setError(null);
    try {
      const current = await readFanProfile();
      if (JSON.stringify(current) !== JSON.stringify(profile)) throw new Error("风扇设置已变化，请重新调整");
      await checked(action());
      await load();
      if (recovery) toast({ kind: "success", title: "风扇设置已恢复" });
    } catch (cause) {
      const message = errText(cause);
      try { await load(); setError(message); }
      catch (refreshError) { setError(`${message}，回读失败：${errText(refreshError)}`); }
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const setMode = (mode: number) => {
    if (!profile || !profile.availableModes.includes(mode) || (mode === CUSTOM_FAN_MODE && !profile.customSupported)) return;
    if (profile.software || mode !== profile.currentMode) void run(() => writeFirmwareMode(profile, mode));
  };
  const changeCurve = (id: FanId, value: CurveDraft) => {
    const fan = profile?.fans.find(item => item.id === id && item.present);
    if (!fan || !profile?.customSupported || profile.software || sameDraft(normalizeDraft(fan, fan), value)) return;
    void run(() => applyFanCurve(id === "cpu" ? value : null, id === "gpu1" ? value : null, id === "gpu2" ? value : null));
  };
  const changeRpm = (id: FanId, points: RpmPoint[]) => {
    const fan = profile?.rpmFans.find(item => item.id === id);
    if (!fan || !profile?.customSupported || profile.software || sameRpmCurve(fan.points, points)) return;
    void run(() => applyRpmFanCurves({ [id]: points }));
  };
  const setOffset = (offset: number) => {
    if (profile && !profile.software && profile.offsetSupported && offsetModeAvailable(profile.currentMode)
      && offset !== profile.offsetPct) void run(() => setFanOffset(offset));
  };
  return { profile, error, busy, readValid, setMode, changeCurve, changeRpm, setOffset,
    restore: () => run(restoreFanProfile, true), restoreDefault: () => run(restoreFanDefault, true),
    retry: () => { void load().catch((cause) => setError(errText(cause))); } };
}
