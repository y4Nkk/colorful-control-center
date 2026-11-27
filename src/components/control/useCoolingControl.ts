import { useCallback, useEffect, useRef, useState } from "react";
import { applyOemPerformanceOptions, CUSTOM_FAN_MODE, readFanProfile,
  readOemPerformanceOptions, restoreFanSettings, saveCustomFanCurves, setFanMode, setFanOffset,
  type FanCurveChanges, type FanProfile, type FanRestoreTarget, type OemPerformanceChange, type OemPerformanceOptions, type OpResult } from "../../api/client";
import { useOemConflict } from "../OemConflictHost";
import { toast } from "../use-toast";
import { errText } from "./format";

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
  const canEditCurves = readValid && !!profile.custom;
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
  const run = async (source: CoolingSource | "curve", action: () => Promise<OpResult>, success?: string) => {
    if (!active || running.current || pendingRead.current) return false;
    if (source === "fan" && (!readValid || profile.blockedReason) ||
        source === "curve" && !canEditCurves ||
        source === "options" && (!options.value || options.error || options.value.blockedReason)) return false;
    running.current = true;
    setWriting(true);
    try {
      const result = await action();
      if (!result.ok) throw new Error(result.message);
      const refreshed = await load();
      setActionError(null);
      const confirmed = refreshed?.[source === "curve" ? "fan" : source] === true;
      if (confirmed && success) toast({ kind: "success", title: success });
      return confirmed;
    } catch (cause) {
      const message = errText(cause);
      await load();
      setActionError(message);
      if (source === "options") conflict.review([message], retry);
      return false;
    } finally {
      running.current = false;
      setWriting(false);
      if (refreshAfterWrite.current) {
        refreshAfterWrite.current = false;
        void load();
      }
      if (source !== "curve") onSettled(source);
    }
  };
  const runFan = async (action: () => Promise<OpResult>) => {
    await run("fan", async () => {
      if (JSON.stringify(await readFanProfile()) !== JSON.stringify(profile)) throw new Error("风扇设置已变化，请重新调整");
      return action();
    });
  };
  return { profile, readValid, canEditCurves, fanError: fan.error, options, actionError, writing, busy: writing || reading, retry,
    apply: (expected: string, change: OemPerformanceChange) => run("options", () => applyOemPerformanceOptions(expected, change)),
    saveCurves: (expected: string, changes: FanCurveChanges) =>
      run("curve", () => saveCustomFanCurves(expected, changes)),
    async restore(target: FanRestoreTarget, expectedRevision: string) {
      await run("fan", () => restoreFanSettings(target, expectedRevision), "风扇设置已恢复");
    },
    ...fanActions(profile, runFan),
  };
}

function fanActions(profile: FanProfile | null, run: (action: () => Promise<OpResult>) => Promise<void>) {
  return {
    setMode(mode: number) {
      const custom = mode === CUSTOM_FAN_MODE;
      if (!profile?.availableModes.includes(mode) || custom && (!profile.custom || profile.customBlockedReason)) return;
      if (profile.software || mode !== profile.currentMode || custom && profile.custom?.pending) return run(() => setFanMode(mode));
    },
    setOffset(offset: number) {
      if (profile?.offsetBlockedReason === null && offset !== profile.offsetPct) {
        return run(() => setFanOffset({ mode: profile.currentMode, slot: profile.offsetSlot }, offset));
      }
    },
  };
}

export type CoolingControl = ReturnType<typeof useCoolingControl>;
