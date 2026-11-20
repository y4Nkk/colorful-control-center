import { useCallback, useEffect, useRef, useState } from "react";
import { applyOemPowerMode, readOemPowerMode, type OemPowerMode, type OpResult } from "../../api/client";
import { useOemConflict } from "../OemConflictHost";
import { gpuModeError } from "./gpu-values";

const errorMessage = (cause: unknown) => cause instanceof Error ? cause.message : String(cause);

export function useOemPowerMode(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<OemPowerMode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<number | null>(null);
  const [reading, setReading] = useState(true);
  const conflict = useOemConflict();
  const loading = useRef(0);
  const running = useRef(false);
  const load = useCallback(async (operationError: string | null = null) => {
    const id = ++loading.current;
    setReading(true);
    try {
      const next = await readOemPowerMode();
      if (id !== loading.current) return;
      setProfile(next);
      setError(operationError);
      return next;
    } catch (cause) {
      if (id !== loading.current) return;
      setProfile(null);
      setError([operationError, errorMessage(cause)].filter(Boolean).join("；"));
    } finally {
      if (id === loading.current) setReading(false);
    }
  }, []);
  useEffect(() => {
    if (!active || running.current) return;
    void load();
    // 切页可作废普通读取；写入后的回读仍须完成，避免保活页停留在忙碌状态。
    return () => { if (!running.current) ++loading.current; };
  }, [active, revision, load]);
  const retry = () => { if (active && !running.current) void load(); };
  const run = async (operation: number, action: (snapshot: OemPowerMode) => Promise<OpResult | null>) => {
    if (!active || running.current || reading || !profile || profile.blockedReason) return;
    ++loading.current;
    running.current = true;
    setPending(operation);
    setError(null);
    let refresh = true;
    try {
      const result = await action(profile);
      if (!result) { refresh = false; return; }
      const next = await load(result.ok ? null : result.message);
      if (result.ok && next && next.currentMode !== operation) {
        setError("设备回读的模式与所选模式不同。");
      }
      if (!result.ok) conflict.review([result.message], retry);
    } catch (cause) {
      const message = errorMessage(cause);
      await load(message);
      conflict.review([message], retry);
    } finally {
      running.current = false;
      setPending(null);
      if (refresh) onSettled();
    }
  };
  return {
    profile, error, reading, pending, retry,
    switchMode: (mode: number, label: string) => run(mode, async snapshot => {
      const target = snapshot.targets.find(candidate => candidate.mode === mode);
      if (!target || !snapshot.availableModes.includes(mode)) throw new Error("未读取到可执行的模式目标。");
      const issue = target.unavailableReason ?? gpuModeError(target.gpu);
      if (issue || !target.revision) throw new Error(issue ?? "模式联动检查尚未就绪。");
      const result = await applyOemPowerMode(target);
      if (!("confirmFnKey" in result)) return result;
      conflict.switchMode(label, () => run(target.mode, async () => {
        const confirmed = await applyOemPowerMode(target, result.confirmFnKey);
        if ("confirmFnKey" in confirmed) throw new Error("原厂 FnKey 状态已变化，请重新选择模式。");
        return confirmed;
      }));
      return null;
    }),
  };
}
