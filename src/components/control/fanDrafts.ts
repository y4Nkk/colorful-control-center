import { useCallback, useState } from "react";
import type { FanId, FanPointSet, FanProfile, RpmPoint } from "../../api/client";
import { normalizeDraft, sameDraft, type CurveDraft } from "./curve";
import { normalizeRpmCurve, sameRpmCurve } from "./rpmCurve";

export const currentOf = (fan: FanPointSet): CurveDraft => normalizeDraft(fan, {
  t2: fan.t2, d2Pct: fan.d2Pct, t3: fan.t3, d3Pct: fan.d3Pct,
});
type Drafts = Partial<Record<FanId, CurveDraft>>;
type RpmDrafts = Partial<Record<FanId, RpmPoint[]>>;
const curvesChanged = (profile: FanProfile, drafts: Drafts) => profile.fans.some((fan) =>
  fan.present && drafts[fan.id] && !sameDraft(currentOf(fan), drafts[fan.id]!));
const rpmChanged = (profile: FanProfile, drafts: RpmDrafts) => profile.rpmFans.some((fan) =>
  drafts[fan.id] && !sameRpmCurve(fan.points, drafts[fan.id]!));

export function useFanDrafts(profile: FanProfile | null) {
  const [drafts, setDrafts] = useState<Drafts>({});
  const [rpmDrafts, setRpmDrafts] = useState<RpmDrafts>({});
  const loadDrafts = useCallback((next: FanProfile, before: FanProfile | null, reset: boolean) => {
    setDrafts((current) => !reset && before && curvesChanged(before, current) ? current :
      Object.fromEntries(next.fans.filter((fan) => fan.present).map((fan) => [fan.id, currentOf(fan)])));
    setRpmDrafts((current) => !reset && before && rpmChanged(before, current) ? current :
      Object.fromEntries(next.rpmFans.map((fan) => [fan.id, fan.points])));
  }, []);
  return { drafts, rpmDrafts, loadDrafts,
    curveDirty: profile ? curvesChanged(profile, drafts) : false,
    rpmDirty: profile ? rpmChanged(profile, rpmDrafts) : false,
    changeCurve: (id: FanId, value: CurveDraft) => setDrafts((current) => ({ ...current, [id]: value })),
    changeRpm: (id: FanId, value: RpmPoint[]) => setRpmDrafts((current) => ({
      ...Object.fromEntries(Object.entries(current).map(([fan, points]) => [fan, normalizeRpmCurve(points!)])), [id]: value,
    })),
  };
}
