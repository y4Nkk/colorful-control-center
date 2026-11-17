import { useEffect, useRef, useState } from "react";
import { getStartupProfile, listProfiles, setStartupProfile, type ProfileMeta, type BackgroundProfileBinding, type ProfileModeApproval } from "../../api/client";
import { loadProfileSelection, type ProfileSelection } from "../../app/useProfileReads";
import { useProfileApplicationState } from "../../app/profile-application";
import { toast } from "../use-toast";

export function useStartupProfile(active: boolean) {
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [selected, setSelected] = useState("");
  const [savedBinding, setSavedBinding] = useState<BackgroundProfileBinding | null>();
  const [bindingIssue, setBindingIssue] = useState<string | null>(null);
  const saved = savedBinding === undefined ? null : savedBinding?.profile ?? "";
  const lastSaved = useRef("");
  const [selection, setSelection] = useState<ProfileSelection | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastRun, setLastRun] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const [readRevision, reread] = useState(0);
  const reportRevision = useRef(0);
  useStartupApplicationReport(reportRevision, setLastRun, setError);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const reportId = ++reportRevision.current;
    setReady(false);
    Promise.allSettled([listProfiles(), getStartupProfile()]).then(([list, current]) => {
      if (cancelled) return;
      setProfiles(list.status === "fulfilled" ? list.value : []);
      if (current.status === "fulfilled") {
        const next = current.value.selection?.profile ?? "", previous = lastSaved.current;
        lastSaved.current = next;
        setSavedBinding(current.value.selection); setBindingIssue(current.value.bindingIssue);
        setSelected(draft => draft === previous ? next : draft);
        if (reportId === reportRevision.current) setLastRun(current.value.lastRun);
      } else {
        setSavedBinding(undefined); setBindingIssue(null); setLastRun(null);
      }
      setError([list, current].flatMap(result => result.status === "rejected" ? [String(result.reason)] : []).join("；") || null);
      setReady(true);
    });
    return () => { cancelled = true; };
  }, [active, readRevision]);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    setSelection(null);
    if (selected) loadProfileSelection(selected, "background").then(next => {
      if (!cancelled) setSelection(next);
    }).catch(error => { if (!cancelled) setError(String(error)); });
    return () => { cancelled = true; };
  }, [selected, active, readRevision]);
  const apply = async () => {
    setBusy(true); setError(null); ++reportRevision.current;
    try {
      const preview = selection?.file ?? null;
      if (selected && (!preview || preview.name !== selected)) throw new Error("请先读取所选启动档案");
      const binding = await setStartupProfile(selected ? preview : null, selected ? selection?.modeAuthorization ?? null : null);
      lastSaved.current = selected;
      setSavedBinding(binding); setBindingIssue(null); toast({ kind: "success", title: "启动档案已保存" });
    } catch (error) { setError(String(error)); }
    finally { setBusy(false); }
  };
  const unchanged = selected === saved && !bindingIssue && sameMode(savedBinding?.mode ?? null, selection?.modeAuthorization ?? null);
  return { profiles, selected, saved, selection, unchanged, error: error ?? bindingIssue, lastRun, busy, ready, setSelected, apply, reread: () => reread(value => value + 1) };
}

function sameMode(grant: BackgroundProfileBinding["mode"], approval: ProfileModeApproval | null) {
  if (!grant || !approval) return !grant && !approval;
  if (grant.identity !== approval.identity) return false;
  const node = approval.target.gpu?.status === "ready" ? approval.target.gpu.value.node : null;
  if (node ? !grant.automaticPowerOff?.includes(node.automaticPowerOff) : grant.automaticPowerOff !== null) return false;
  const driver = approval.clocks.driver?.identity;
  return !driver || (grant.driver === null ? grant.driverDeferred : grant.driver === driver);
}

function useStartupApplicationReport(revision: { current: number }, update: (message: string | null) => void, reportError: (error: string) => void) {
  const { report } = useProfileApplicationState();
  const startup = report?.origin === "startup", id = report?.id;
  const revisionId = report?.revision;
  useEffect(() => {
    if (!startup) return;
    const generation = ++revision.current;
    let disposed = false;
    getStartupProfile().then(result => { if (!disposed && generation === revision.current) update(result.lastRun); })
      .catch(error => { if (!disposed && generation === revision.current) reportError(String(error)); });
    return () => { disposed = true; };
  }, [startup, id, revisionId, revision, update, reportError]);
}
