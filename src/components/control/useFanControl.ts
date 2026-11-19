import { useCallback, useEffect, useRef, useState } from "react";
import { applyFanCurve, applyRpmFanCurves, readFanProfile, restoreFanDefault, restoreFanProfile, setFanMode, setFanOffset,
  type FanId, type FanProfile, type RpmPoint } from "../../api/client";
import { toast } from "../use-toast";
import type { CurveDraft } from "./curve";
import { sameRpmCurve } from "./rpmCurve";
import { useFanDrafts } from "./fanDrafts";

const checked = async (action: Promise<{ ok: boolean; message: string }>) => {
  const result = await action; if (!result.ok) throw new Error(result.message);
};

export function useFanControl(active: boolean, externalBusy: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<FanProfile | null>(null);
  const curves = useFanDrafts(profile);
  const [offset, setOffset] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [readValid, setReadValid] = useState(false);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const previous = useRef<FanProfile | null>(null);
  const loadId = useRef(0);
  const load = useCallback(async (reset: boolean) => {
    const id = ++loadId.current;
    let next: FanProfile;
    try { next = await readFanProfile(); }
    catch (cause) { if (id === loadId.current) setReadValid(false); throw cause; }
    if (id !== loadId.current) return;
    const before = previous.current;
    curves.loadDrafts(next, before, reset);
    setOffset((current) => !reset && before && current !== before.offsetPct ? current : next.offsetPct);
    previous.current = next; setProfile(next); setReadValid(true); setError(null);
  }, [curves.loadDrafts]);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    if (!running.current) void load(false).catch((cause) => { if (!disposed) setError(String(cause)); });
    return () => { disposed = true; if (!running.current) ++loadId.current; };
  }, [active, revision, load]);
  const softwareRunning = profile?.software != null;
  useEffect(() => {
    if (!active || !softwareRunning || externalBusy) return;
    const timer = window.setInterval(() => { if (!running.current) void load(false).catch((cause) => setError(String(cause))); }, 5000);
    return () => window.clearInterval(timer);
  }, [active, softwareRunning, externalBusy, load]);
  const run = async (action: () => Promise<void>, reset = true) => {
    if (!active || running.current || externalBusy || !readValid || !profile || profile.blockedReason) return;
    running.current = true; ++loadId.current; setBusy(true); setError(null);
    try { await action(); await load(reset); toast({ kind: "success", title: "风扇设置已写入" }); }
    catch (cause) {
      const message = String(cause);
      try { await load(false); setError(message); }
      catch (refreshError) { setError(`${message}；回读失败：${String(refreshError)}`); }
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const { drafts, rpmDrafts, curveDirty, rpmDirty } = curves;
  const dirty = profile != null && (curveDirty || rpmDirty || offset !== profile.offsetPct);
  const apply = () => run(async () => {
    if (!profile) return;
    if (curveDirty) await checked(applyFanCurve(drafts.cpu ?? null, drafts.gpu1 ?? null, drafts.gpu2 ?? null));
    if (rpmDirty) await checked(applyRpmFanCurves(Object.fromEntries(profile.rpmFans
      .filter((fan) => rpmDrafts[fan.id] && !sameRpmCurve(fan.points, rpmDrafts[fan.id]!))
      .map((fan) => [fan.id, rpmDrafts[fan.id]!]))));
    if (profile.software && !curveDirty && !rpmDirty) await checked(setFanMode(profile.currentMode));
    if (offset !== profile.offsetPct) await checked(setFanOffset(offset));
  });
  const changeCurve = (id: FanId, value: CurveDraft) => {
    if (running.current) return;
    curves.changeCurve(id, value);
  };
  const changeRpm = (id: FanId, value: RpmPoint[]) => {
    if (running.current) return;
    curves.changeRpm(id, value);
  };
  const setMode = (value: number) => {
    if (!profile || !profile.availableModes.includes(value) || value === 6 && !profile.customSupported ||
      value === profile.currentMode && !profile.software) return;
    void run(() => checked(setFanMode(value)), false);
  };
  return { profile, drafts, rpmDrafts, setMode, offset, setOffset, error, busy: busy || externalBusy, readValid, dirty,
    curveChanged: curveDirty || rpmDirty, apply, changeCurve, changeRpm,
    refresh: () => load(true),
    restore: () => run(() => checked(restoreFanProfile())),
    restoreDefault: () => run(() => checked(restoreFanDefault())),
    retry: () => { void load(false).catch((cause) => setError(String(cause))); } };
}
