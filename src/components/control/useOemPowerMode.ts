import { useCallback, useEffect, useRef, useState } from "react";
import { applyOemPowerMode, readOemPowerMode, restoreOemPowerMode,
  type FnKeyModeConfirmation, type OemModeTarget, type OemPowerMode, type OpResult } from "../../api/client";
import { gpuModeError } from "./gpu-values";

const errorMessage = (cause: unknown) => cause instanceof Error ? cause.message : String(cause);
type FnKeyPrompt = { target: OemModeTarget; confirmation: FnKeyModeConfirmation };

export function useOemPowerMode(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<OemPowerMode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<number | "restore" | null>(null);
  const [reading, setReading] = useState(true);
  const [fnKeyPrompt, setFnKeyPrompt] = useState<FnKeyPrompt | null>(null);
  const loading = useRef(0);
  const running = useRef(false);
  const activation = useRef(0);
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
  useEffect(() => {
    if (!active) setFnKeyPrompt(null);
    return () => { ++activation.current; };
  }, [active]);
  const run = async (operation: number | "restore", action: (snapshot: OemPowerMode) => Promise<OpResult | null>) => {
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
      if (result.ok && typeof operation === "number" && next && next.currentMode !== operation) {
        setError("设备回读的模式与所选模式不同。");
      }
    } catch (cause) {
      await load(errorMessage(cause));
    } finally {
      running.current = false;
      setPending(null);
      if (refresh) onSettled();
    }
  };
  return {
    profile, error, reading, pending, fnKeyPrompt,
    retry: () => { if (active && !running.current) void load(); },
    switchMode: (mode: number) => run(mode, async snapshot => {
      const initialActivation = activation.current;
      const target = snapshot.targets.find(candidate => candidate.mode === mode);
      if (!target || !snapshot.availableModes.includes(mode)) throw new Error("未读取到可执行的模式目标。");
      const issue = target.unavailableReason ?? gpuModeError(target.gpu);
      if (issue || !target.revision) throw new Error(issue ?? "模式联动检查尚未就绪。");
      const result = await applyOemPowerMode(target);
      if (!("confirmFnKey" in result)) return result;
      if (initialActivation === activation.current) setFnKeyPrompt({ target, confirmation: result.confirmFnKey });
      return null;
    }),
    cancelFnKey: () => { if (!running.current) setFnKeyPrompt(null); },
    confirmFnKey: () => {
      if (!fnKeyPrompt) return;
      const { target, confirmation } = fnKeyPrompt;
      return run(target.mode, async () => {
        try {
          const result = await applyOemPowerMode(target, confirmation);
          if ("confirmFnKey" in result) throw new Error("原厂 FnKey 状态已变化，请重新选择模式。");
          return result;
        } finally { setFnKeyPrompt(null); }
      });
    },
    restore: (expectedRevision: string) => run("restore", snapshot => {
      if (!snapshot.canRestore || snapshot.restore?.revision !== expectedRevision) throw new Error("恢复目标已变化，请重新读取。");
      return restoreOemPowerMode(expectedRevision);
    }),
  };
}
