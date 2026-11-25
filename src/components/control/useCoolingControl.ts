import { useCallback, useEffect, useRef, useState } from "react";
import { applyFanCurve, applyOemPerformanceOptions, applyRpmFanCurves, CUSTOM_FAN_MODE, readFanProfile,
  readOemPerformanceOptions, restoreFanSettings, setFanMode, setFanOffset,
  type FanId, type FanProfile, type FanRestoreTarget, type OemPerformanceChange, type OemPerformanceOptions, type OpResult, type RpmPoint } from "../../api/client";
import { useOemConflict } from "../OemConflictHost";
import { toast } from "../use-toast";
import { normalizeDraft, sameDraft, type CurveDraft } from "./curve";
import { errText } from "./format";
import { sameRpmCurve } from "./rpmCurve";

type CoolingSource = "fan" | "options";
type Reading<T> = { value: T | null; error: string | null };
const empty = { value: null, error: null };

function readback<T>(previous: Reading<T>, result: PromiseSettledResult<T>): Reading<T> {
  return result.status === "fulfilled" ? { value: result.value, error: null }
    : { value: previous.value, error: errText(result.reason) };
}

export function useCoolingControl(active: boolean, revision: number, onSettled: (source: CoolingSource) => void) {
  const [{ fan, options }, setSnapshot] = useState<{ fan: Reading<FanProfile>; options: Reading<OemPerformanceOptions> }>({ fan: empty, options: empty });
  const [actionError, setActionError] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);
  const [reading, setReading] = useState(false);
  const running = useRef(false);
  const generation = useRef(0);
  const pendingRead = useRef(false);
  const refreshAfterWrite = useRef(false);
  const profile = fan.value;
  const readValid = profile !== null && fan.error === null;
  const load = useCallback(async () => {
    const id = ++generation.current;
    pendingRead.current = true;
    setReading(true);
    const [fanResult, optionsResult] = await Promise.allSettled([readFanProfile(), readOemPerformanceOptions()]);
    if (id !== generation.current) return;
    pendingRead.current = false;
    setReading(false);
    setSnapshot(previous => ({ fan: readback(previous.fan, fanResult), options: readback(previous.options, optionsResult) }));
    return { fan: fanResult.status === "fulfilled", options: optionsResult.status === "fulfilled" };
  }, []);
  useEffect(() => {
    if (!active) return;
    if (running.current) refreshAfterWrite.current = true;
    else void load();
  }, [active, revision, load]);
  const softwareRunning = !!profile?.software;
  useEffect(() => {
    if (!active || !softwareRunning) return;
    const timer = window.setInterval(() => { if (!running.current && !pendingRead.current) void load(); }, 5000);
    return () => window.clearInterval(timer);
  }, [active, softwareRunning, load]);
  const conflict = useOemConflict();
  const retry = () => { if (active && !running.current) { setActionError(null); void load(); } };
  const run = async (source: CoolingSource, action: () => Promise<OpResult>, success?: string) => {
    if (!active || running.current || pendingRead.current) return;
    if (source === "fan" ? !readValid || profile?.blockedReason : !options.value || options.error || options.value.blockedReason) return;
    running.current = true;
    setWriting(true);
    try {
      const result = await action();
      if (!result.ok) throw new Error(result.message);
      const refreshed = await load();
      setActionError(null);
      if (refreshed?.[source] && success) toast({ kind: "success", title: success });
    } catch (cause) {
      const message = errText(cause);
      await load();
      setActionError(message);
      if (source === "options") conflict.review([message], retry);
    } finally {
      running.current = false;
      setWriting(false);
      if (refreshAfterWrite.current) {
        refreshAfterWrite.current = false;
        void load();
      }
      onSettled(source);
    }
  };
  return { profile, readValid, fanError: fan.error, options, actionError, writing, busy: writing || reading, retry,
    apply: (expected: string, change: OemPerformanceChange) => run("options", () => applyOemPerformanceOptions(expected, change)),
    restore: (target: FanRestoreTarget, expectedRevision: string) =>
      run("fan", () => restoreFanSettings(target, expectedRevision), "风扇设置已恢复"),
    ...fanActions(profile, action => run("fan", async () => {
      if (JSON.stringify(await readFanProfile()) !== JSON.stringify(profile)) {
        throw new Error("风扇设置已变化，请重新调整");
      }
      return action();
    })),
  };
}

function fanActions(profile: FanProfile | null, run: (action: () => Promise<OpResult>) => Promise<void>) {
  return {
    setMode(mode: number) {
      if (!profile?.availableModes.includes(mode) || mode === CUSTOM_FAN_MODE && (!profile.customSupported || profile.customBlockedReason)) return;
      if (profile.software || mode !== profile.currentMode) return run(() => setFanMode(mode));
    },
    changeCurve(id: FanId, value: CurveDraft) {
      const fan = profile?.fans.find(item => item.id === id && item.present);
      if (!fan || !profile?.customSupported || profile.customBlockedReason || profile.software || sameDraft(normalizeDraft(fan, fan), value)) return;
      return run(() => applyFanCurve(id === "cpu" ? value : null, id === "gpu1" ? value : null, id === "gpu2" ? value : null));
    },
    changeRpm(id: FanId, points: RpmPoint[]) {
      const fan = profile?.rpmFans.find(item => item.id === id);
      if (!fan || !profile?.customSupported || profile.customBlockedReason || profile.software || sameRpmCurve(fan.points, points)) return;
      return run(() => applyRpmFanCurves({ [id]: points }));
    },
    setOffset(offset: number) {
      if (profile?.offsetBlockedReason === null && offset !== profile.offsetPct) {
        return run(() => setFanOffset({ mode: profile.currentMode, slot: profile.offsetSlot }, offset));
      }
    },
  };
}

export type CoolingControl = ReturnType<typeof useCoolingControl>;
