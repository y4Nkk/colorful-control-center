import { useEffect, useRef, useState } from "react";
import {
  applyProfile, deleteProfile, exportProfile, loadProfile, previewProfileApplication, previewProfileImport, saveProfile,
  type ManualProfilePreview, type ProfileApplication, type ProfileFile, type ProfileMeta, type ProfileSettings,
} from "../api/client";
import { useOemConflict } from "../components/OemConflictHost";
import { toast } from "../components/use-toast";
import { useProfileList } from "./useProfileReads";
import { runProfileApplication, useProfileApplicationState } from "./profile-application";
import { useProfileReferences } from "./useProfileReferences";

interface ReviewedProfile { file: ProfileFile; review: ManualProfilePreview }
type Target = { name: string };

export function useProfiles(active: boolean, requested?: Target | null) {
  const conflict = useOemConflict();
  const [target, choose] = useState<Target | null>(null);
  const [imported, setImported] = useState<ProfileFile | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const running = useRef(false);
  const consumed = useRef<Target | null>(null);
  const catalog = useProfileList(active, setListError);
  const refs = useProfileReferences(active, catalog.profiles);
  const { report, busy: applying } = useProfileApplicationState();
  const pending = !!report?.pending;
  const working = busy || applying;
  const detail = useManualProfile(active && !imported && !applying && !pending, target, report);
  useEffect(() => {
    if (!active || working || catalog.status !== "ready") return;
    if (requested && consumed.current !== requested) {
      consumed.current = requested;
      setImported(null);
      choose(requested);
    } else if (!target && !imported && catalog.profiles.length) {
      choose({ name: catalog.profiles[0].name });
    }
  }, [active, working, catalog.status, catalog.profiles, requested, imported, target]);

  const select = (name: string) => {
    setError(null);
    setImported(null);
    choose({ name });
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
      conflict.review([message], target ? () => select(target.name) : undefined);
      return false;
    } finally { running.current = false; setBusy(false); }
  };
  const persist = (name: string, settings: ProfileSettings, existing: ProfileMeta | undefined, source: "save" | "import") =>
    run(async () => {
      const result = await saveProfile(name, settings, existing?.revision ?? null);
      await reload();
      if (!result.ok) throw new Error(result.message);
      select(name);
      toast({ kind: "success", title: source === "import" ? "档案已导入" : "档案已保存" });
    });
  const remove = (profile: ProfileMeta) => run(async () => {
    const result = await deleteProfile(profile.name, profile.revision);
    await reload();
    if (!result.ok) { choose({ name: profile.name }); throw new Error(result.message); }
    choose(null);
    toast({ kind: "success", title: "档案已删除" });
  });
  const stageImport = (file: File) => run(async () => {
    if (file.size > 1024 * 1024) throw new Error("档案不能超过 1 MB");
    const candidate = await previewProfileImport(await file.text());
    setImported(candidate);
  });
  const apply = (selection: ReviewedProfile) => run(async () => {
    const { file, review } = selection;
    if (review.blocked || pending) return;
    const result = await runProfileApplication(() => applyProfile(file, review.modeTarget, review.displayReview,
      review.gpuPowerReview, review.graphicsModeReview));
    conflict.review([result.pending?.error, ...result.steps.filter(step => !step.ok).map(step => step.message)], () => select(file.name));
    presentApplication(result, setError);
  });
  return { ...catalog, ...detail, reload, select, selectedName: target?.name, imported, dismissImport: () => setImported(null),
    ...refs, error, clearError: () => setError(null), listError, busy: working, report, pending, persist, remove, stageImport, apply,
    download: (name: string) => run(() => downloadProfile(name)) };
}

function useManualProfile(active: boolean, target: Target | null, report: ProfileApplication | null) {
  const [value, setValue] = useState<{ target: Target; selection: ReviewedProfile | null; issue: string | null } | null>(null);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    if (!active || !target) return;
    const { name } = target;
    let disposed = false;
    setLoading(true);
    async function load() {
      const file = await loadProfile(name);
      const review = await previewProfileApplication(file.settings).catch((error): ManualProfilePreview => ({
        before: null, modeTarget: null, displayReview: null, gpuPowerReview: null, graphicsModeReview: null, blocked: String(error),
      }));
      return { file, review };
    }
    load().then(selection => { if (!disposed) setValue({ target, selection, issue: null }); })
      .catch(error => { if (!disposed) setValue({ target, selection: null, issue: String(error) }); })
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [active, target, report?.id, report?.revision]);
  const current = value?.target === target ? value : null;
  return { selection: current?.selection ?? null, readError: current?.issue ?? null, loading: loading || !!target && !current };
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
