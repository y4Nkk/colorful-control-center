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
  const [mode, setModeValue] = useState(0);
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
    setModeValue((current) => !reset && before && current !== before.currentMode ? current : next.currentMode);
    setOffset((current) => !reset && before && current !== before.offsetPct ? current : next.offsetPct);
    previous.current = next; setProfile(next); setReadValid(true); setError(null);
  }, [curves.loadDrafts]);
  useEffect(() => {
    if (!active) return;
    let disposed = false;
    void load(false).catch((cause) => { if (!disposed) setError(String(cause)); });
    return () => { disposed = true; ++loadId.current; };
  }, [active, revision, load]);
  const softwareRunning = profile?.software != null;
  useEffect(() => {
    if (!active || !softwareRunning || externalBusy) return;
    const timer = window.setInterval(() => { if (!running.current) void load(false).catch((cause) => setError(String(cause))); }, 5000);
    return () => window.clearInterval(timer);
  }, [active, softwareRunning, externalBusy, load]);
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null);
    try { await action(); await load(true); toast({ kind: "success", title: "风扇设置已写入" }); }
    catch (cause) {
      const message = String(cause);
      try { await load(false); setError(message); }
      catch (refreshError) { setError(`${message}；回读失败：${String(refreshError)}`); }
    } finally { running.current = false; setBusy(false); onSettled(); }
  };
  const { drafts, rpmDrafts, curveDirty, rpmDirty } = curves;
  const dirty = profile != null && (profile.software != null || curveDirty || rpmDirty || mode !== profile.currentMode || offset !== profile.offsetPct);
  const apply = () => run(async () => {
    if (!profile) return;
    if (curveDirty) await checked(applyFanCurve(drafts.cpu ?? null, drafts.gpu1 ?? null, drafts.gpu2 ?? null));
    const writeRpm = profile.curveKind === "rpmTable" && (rpmDirty || (mode === 6 && profile.currentMode !== 6));
    if (writeRpm) await checked(applyRpmFanCurves(Object.fromEntries(profile.rpmFans
      .filter((fan) => !rpmDirty || !sameRpmCurve(fan.points, rpmDrafts[fan.id]!))
      .map((fan) => [fan.id, rpmDrafts[fan.id]!]))));
    if (mode !== (curveDirty || writeRpm ? 6 : profile.currentMode) || (profile.software && !curveDirty && !writeRpm)) await checked(setFanMode(mode));
    if (offset !== profile.offsetPct) await checked(setFanOffset(offset));
  });
  const changeCurve = (id: FanId, value: CurveDraft) => {
    if (running.current) return;
    curves.changeCurve(id, value); setModeValue(6);
  };
  const changeRpm = (id: FanId, value: RpmPoint[]) => {
    if (running.current) return;
    curves.changeRpm(id, value); setModeValue(6);
  };
  const setMode = (value: number) => {
    if (value === 6 && profile?.curveKind === "rpmTable") curves.ensureRpmTargets();
    setModeValue(value);
  };
  return { profile, drafts, rpmDrafts, mode, setMode, offset, setOffset, error, busy, readValid, dirty, apply, changeCurve, changeRpm,
    refresh: () => load(true),
    restore: () => run(() => checked(restoreFanProfile())),
    restoreDefault: () => run(() => checked(restoreFanDefault())),
    retry: () => { void load(false).catch((cause) => setError(String(cause))); } };
}
