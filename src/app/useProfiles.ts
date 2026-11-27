import { useRef, useState } from "react";
import {
  applyBuiltinProfile, applyProfile, deleteProfile, exportProfile, previewProfileImport, saveProfile,
  type ProfileApplication, type ProfileFile, type ProfileMeta, type ProfileSettings,
} from "../api/client";
import { useOemConflict } from "../components/OemConflictHost";
import { toast } from "../components/use-toast";
import { useProfileList } from "./useProfileReads";
import { runProfileApplication, useProfileApplicationState } from "./profile-application";
import { useProfileReferences } from "./useProfileReferences";
import { useBuiltinProfiles } from "./useBuiltinProfiles";
import { useProfilePreview, useProfileTarget, type ProfileRequest, type ProfileTarget, type ReviewedProfile } from "./useProfilePreview";

export function useProfiles(active: boolean, requested?: ProfileRequest | null) {
  const conflict = useOemConflict();
  const [imported, setImported] = useState<ProfileFile | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const running = useRef(false);
  const catalog = useProfileList(active, setListError);
  const builtin = useBuiltinProfiles(active);
  const refs = useProfileReferences(active, catalog.profiles);
  const { report, busy: applying } = useProfileApplicationState();
  const pending = !!report?.pending;
  const working = busy || applying;
  const savedDefault = catalog.profiles[0];
  const fallback: ProfileTarget | null = !imported && savedDefault ? { kind: "saved", name: savedDefault.name } : null;
  const { target, choose } = useProfileTarget(active && !working && catalog.status === "ready", requested, fallback, setImported);
  const detail = useProfilePreview(active && !imported && !working && !pending, target, target?.kind === "builtin" ? builtin.choice : null, report);

  const select = (target: ProfileTarget) => {
    setError(null);
    setImported(null);
    choose({ ...target });
  };
  const retry = () => {
    if (target?.kind === "builtin") void builtin.reload();
    if (target) select(target);
  };
  const reload = async () => {
    setListError(null);
    try { await catalog.reload(); }
    catch (error) { setListError(String(error)); }
  };
  const run = async (action: () => Promise<void>) => {
    if (!active || running.current || applying) return false;
    running.current = true;
    setBusy(true); setError(null);
    try { await action(); return true; }
    catch (error) {
      const message = String(error);
      setError(message);
      conflict.review([message], retry);
      return false;
    } finally { running.current = false; setBusy(false); }
  };
  const persist = (name: string, settings: ProfileSettings, existing: ProfileMeta | undefined, source: "save" | "import" | "copy") =>
    run(async () => {
      const result = await saveProfile(name, settings, existing?.revision ?? null);
      await reload();
      if (!result.ok) throw new Error(result.message);
      select({ kind: "saved", name });
      toast({ kind: "success", title: source === "import" ? "档案已导入" : source === "copy" ? "档案副本已保存" : "档案已保存" });
    });
  const remove = (profile: ProfileMeta) => run(async () => {
    const result = await deleteProfile(profile.name, profile.revision);
    await reload();
    if (!result.ok) { choose({ kind: "saved", name: profile.name }); throw new Error(result.message); }
    choose(undefined);
    toast({ kind: "success", title: "档案已删除" });
  });
  const stageImport = (file: File) => run(async () => {
    if (file.size > 1024 * 1024) throw new Error("档案不能超过 1 MB");
    const candidate = await previewProfileImport(await file.text());
    setImported(candidate);
  });
  const apply = (selection: ReviewedProfile) => run(async () => {
    const { file, review, builtin } = selection;
    if (review.blocked || pending || detail.loading || detail.selection !== selection) return;
    const result = await runProfileApplication(() => builtin
      ? applyBuiltinProfile(builtin, file, review.modeTarget, review.displayReview)
      : applyProfile(file, review.modeTarget, review.displayReview, review.gpuPowerReview, review.graphicsModeReview));
    conflict.review([result.pending?.error, ...result.steps.filter(step => !step.ok).map(step => step.message)], retry);
    presentApplication(result, setError);
  });
  return { ...catalog, ...detail, reload, retry, select, target, builtin, imported, dismissImport: () => setImported(null),
    dismissPreview: () => { choose(null); setError(null); },
    ...refs, error, clearError: () => setError(null), listError, busy: working, report, pending, persist, remove, stageImport, apply,
    download: (name: string) => run(() => downloadProfile(name)) };
}

function presentApplication(result: ProfileApplication, reportError: (message: string) => void) {
  if (result.pending) {
    const recovery = result.pending.error || result.steps.some(step => !step.ok);
    toast({ kind: recovery ? "warning" : "info", title: recovery ? "档案显示恢复待处理" : "请确认档案显示试用" });
    return;
  }
  const failed = result.steps.find(step => !step.ok);
  if (failed && result.targetsApplied) {
    reportError(`目标设置已应用；${failed.message}；详情见执行结果`);
    toast({ kind: "warning", title: "目标设置已应用，部分收尾待处理" });
    return;
  }
  if (failed) throw new Error(result.steps.some(step => step.step.startsWith("恢复") && !step.ok)
    ? `应用失败，部分设置恢复失败；${failed.message}；请查看执行结果并重试恢复`
    : `应用未完成：${failed.message}；详情见执行结果`);
  if (!result.targetsApplied) throw new Error("档案尚未确认全部目标；详情见执行结果");
  toast({ kind: "success", title: `${result.name} 已应用` });
}

async function downloadProfile(name: string) {
  const data = await exportProfile(name);
  const url = URL.createObjectURL(new Blob([data], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${name}.json`;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
