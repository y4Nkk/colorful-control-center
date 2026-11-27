import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import {
  loadProfile, previewBuiltinProfile, previewProfileApplication,
  type BuiltinProfileId, type BuiltinProfileRequest, type BuiltinRefreshChoice,
  type ManualProfilePreview, type ProfileApplication, type ProfileFile,
} from "../api/client";

export type ProfileTarget = { kind: "saved"; name: string } | { kind: "builtin"; id: BuiltinProfileId };
export interface ReviewedProfile { file: ProfileFile; review: ManualProfilePreview; builtin: BuiltinProfileRequest | null }
export type ProfileRequest = { name: string };

export function useProfileTarget(ready: boolean, requested: ProfileRequest | null | undefined, fallback: ProfileTarget | null,
  setImported: Dispatch<SetStateAction<ProfileFile | null>>) {
  const [selection, choose] = useState<ProfileTarget | null>();
  const consumed = useRef<ProfileRequest | null>(null);
  useEffect(() => {
    if (!ready) return;
    if (requested && consumed.current !== requested) {
      consumed.current = requested;
      setImported(null);
      choose({ kind: "saved", name: requested.name });
    } else if (selection === undefined && fallback) {
      choose(fallback);
    }
  }, [ready, requested, fallback, setImported, selection]);
  return { target: selection ?? null, choose };
}

export function useProfilePreview(active: boolean, target: ProfileTarget | null, refresh: BuiltinRefreshChoice | null,
  report: ProfileApplication | null) {
  const [value, setValue] = useState<{ target: ProfileTarget; refresh: BuiltinRefreshChoice | null; selection: ReviewedProfile | null; issue: string | null } | null>(null);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    if (!active || !target) return;
    let disposed = false;
    const finish = (selection: ReviewedProfile | null, issue: string | null) => {
      if (!disposed) setValue({ target, refresh, selection, issue });
    };
    setLoading(true);
    reviewTarget(target, refresh).then(selection => finish(selection, null))
      .catch(error => finish(null, String(error)))
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [active, target, refresh, report?.id, report?.revision]);
  const current = value?.target === target && value.refresh === refresh ? value : null;
  return { selection: current?.selection ?? null, readError: current?.issue ?? null, loading: loading || !!target && !current };
}

async function reviewTarget(target: ProfileTarget, refresh: BuiltinRefreshChoice | null): Promise<ReviewedProfile> {
  if (target.kind === "builtin") {
    const builtin = { id: target.id, refresh };
    return { ...await previewBuiltinProfile(builtin), builtin };
  }
  const file = await loadProfile(target.name);
  const review = await previewProfileApplication(file.settings).catch((error): ManualProfilePreview => ({
    before: null, modeTarget: null, displayReview: null, gpuPowerReview: null, graphicsModeReview: null, blocked: String(error),
  }));
  return { file, review, builtin: null };
}
