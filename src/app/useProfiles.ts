import { useCallback, useEffect, useRef, useState } from "react";
import { applyProfile, captureProfileSettings, deleteProfile, exportProfile, importProfile,
  previewProfileImport, saveProfile, type ProfileApplication } from "../api/client";
import { toast } from "../components/use-toast";
import { useProfileList, loadProfileSelection, type ProfileSelection } from "./useProfileReads";
import { runProfileApplication, useProfileApplicationState } from "./profile-application";

export function useProfiles(active: boolean, requested?: { name: string } | null) {
  const [selection, setSelection] = useState<ProfileSelection | null>(null);
  const preview = selection?.file ?? null;
  const modeTarget = selection?.modeTarget ?? null;
  const displayReview = selection?.displayReview ?? null;
  const gpuPowerReview = selection?.gpuPowerReview ?? null;
  const graphicsModeReview = selection?.graphicsModeReview ?? null;
  const application = useProfileApplicationState();
  const [imported, setImported] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const loadId = useRef(0);
  const consumedRequest = useRef<typeof requested>(null);
  const { profiles, reload } = useProfileList(active, setError);
  const select = useCallback(async (target: string) => {
    const id = ++loadId.current;
    setImported(null); setError(null);
    try { const next = await loadProfileSelection(target, "manual"); if (id === loadId.current) { setSelection(next); setError([next.modeError, next.displayError, next.gpuPowerError, next.graphicsModeError].filter(Boolean).join("；") || null); } }
    catch (e) { if (id === loadId.current) { setError(String(e)); setSelection(null); } }
  }, []);
  useEffect(() => {
    if (!requested || busy || running.current || consumedRequest.current === requested) return;
    consumedRequest.current = requested;
    void select(requested.name);
  }, [requested, busy, select]);
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    ++loadId.current;
    running.current = true; setBusy(true); setError(null);
    try { await action(); }
    catch (e) { setError(String(e)); }
    finally { running.current = false; setBusy(false); }
  };
  const save = () => run(async () => {
    const result = await saveProfile(name.trim(), await captureProfileSettings());
    if (!result.ok) throw new Error(result.message);
    await reload(); await select(name.trim()); toast({ kind: "success", title: "档案已保存" });
  });
  const apply = () => run(async () => {
    if (!preview || imported) return;
    const result = await runProfileApplication(() => applyProfile(preview, modeTarget, displayReview, gpuPowerReview, graphicsModeReview));
    presentApplication(result, setError);
  });
  const remove = () => run(async () => {
    if (!preview || imported) return;
    const result = await deleteProfile(preview.name);
    if (!result.ok) throw new Error(result.message);
    setSelection(null); await reload(); toast({ kind: "success", title: result.message });
  });
  const download = () => run(() => !preview || imported ? Promise.resolve() : downloadProfile(preview.name));
  const stageImport = async (file: File) => {
    if (running.current) return;
    const id = ++loadId.current;
    try {
      if (file.size > 1024 * 1024) throw new Error("档案不能超过 1 MB");
      const text = await file.text();
      const candidate = await previewProfileImport(text);
      if (id !== loadId.current || running.current) return;
      setImported(text); setSelection({ file: candidate, modeTarget: null, modeAuthorization: null, modeError: null, displayReview: null, displayError: null, gpuPowerReview: null, gpuPowerError: null, graphicsModeReview: null, graphicsModeError: null }); setError(null);
    } catch (e) { if (id === loadId.current && !running.current) setError(String(e)); }
  };
  const confirmImport = () => run(async () => {
    if (!imported) return;
    const result = await importProfile(imported);
    if (!result.ok) throw new Error(result.message);
    const target = preview?.name; setImported(null); await reload(); if (target) await select(target);
  });
  return { profiles, preview, modeTarget, displayReview, gpuPowerReview, graphicsModeReview, application: application.report, applicationPending: !!application.report?.pending, imported, name, setName, error, busy: busy || application.busy,
    select, save, apply, remove, download, stageImport, confirmImport };
}

function presentApplication(result: ProfileApplication, reportError: (message: string) => void) {
  if (result.pending) {
    const recovery = result.pending.error || result.steps.some(step => !step.ok);
    toast({ kind: recovery ? "warning" : "info", title: recovery ? "档案显示恢复待处理" : "请确认档案显示试用", description: "请查看全局显示确认与执行结果" }); return;
  }
  const failed = result.steps.filter(step => !step.ok);
  if (failed.length && result.targetsApplied) {
    reportError(`目标设置已应用；${failed[0].message}；详情见执行结果`);
    toast({ kind: "warning", title: "目标设置已应用，部分收尾待处理" }); return;
  }
  if (failed.length) throw new Error(result.steps.some(step => step.step.startsWith("恢复") && !step.ok)
    ? `应用失败，部分设置恢复失败；${failed[0].message}；请查看执行结果并重试恢复`
    : `应用未完成：${failed[0].message}；详情见执行结果`);
  if (!result.targetsApplied) throw new Error("档案尚未确认全部目标；详情见执行结果");
  toast({ kind: "success", title: `${result.name} 已应用` });
}

async function downloadProfile(name: string) {
  const data = await exportProfile(name);
  const url = URL.createObjectURL(new Blob([data], { type: "application/json" }));
  const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${name}.json`;
  anchor.click(); URL.revokeObjectURL(url);
}
