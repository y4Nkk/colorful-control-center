import { useEffect, useRef, useState } from "react";
import { applyWindowsPowerPlan, readWindowsPowerPlans, type WindowsPowerPlan } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { useDeviceSettings } from "./useDeviceSettings";

const TITLE = "Windows 电源计划";
const PICKER = "选择计划";
const planName = (plan: WindowsPowerPlan) => plan.name.trim() || plan.id;
type Draft = { revision: string; id: string } | "stale" | null;

function useWindowsPlanProfile(active: boolean, revision: number, onPlanChanged: () => void) {
  const state = useDeviceSettings(readWindowsPowerPlans, active, revision, 5000);
  const { loaded, refresh } = state;
  const activeId = state.profile?.activeId ?? null;
  const lastRead = useRef<string | null | undefined>(undefined);
  useEffect(() => {
    if (!active || !loaded || lastRead.current === activeId) return;
    const previous = lastRead.current;
    lastRead.current = activeId;
    if (previous !== undefined) onPlanChanged();
  }, [active, activeId, loaded, onPlanChanged]);
  useEffect(() => {
    if (!active) return;
    const onFocus = () => { void refresh(); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [active, refresh]);
  return state;
}

export function WindowsPowerPlans({ active, revision, onPlanChanged }: {
  active: boolean; revision: number; onPlanChanged: () => void;
}) {
  const { profile, busy, loaded, error, run, reread } = useWindowsPlanProfile(active, revision, onPlanChanged);
  const [draft, setDraft] = useState<Draft>(null);
  const selection = draft === "stale" ? null : draft;
  const stale = selection !== null && selection.revision !== profile?.revision;
  if (stale) setDraft("stale");
  const plans = profile?.plans ?? [];
  const current = plans.find(plan => plan.id === profile?.activeId);
  const target = !stale && selection ? plans.find(plan => plan.id === selection.id) : undefined;
  const locked = !active || busy || !profile || !!profile.blockedReason;
  const nameCounts = new Map<string, number>();
  for (const plan of plans) {
    const name = planName(plan);
    nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  }
  const label = (plan: WindowsPowerPlan) => (nameCounts.get(planName(plan)) ?? 0) > 1
    ? `${planName(plan)} · ${plan.id}` : planName(plan);

  const apply = async (id: string) => {
    if (!profile || locked) return;
    await run(async () => {
      await applyWindowsPowerPlan(profile.revision, id);
      setDraft(null);
    }, "Windows 电源计划已切换");
  };

  return <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="windows-power-plan-heading">
    <div className="ccc-panel-head">
      <h2 id="windows-power-plan-heading" className="ccc-panel-title"><Icon name="power" />{TITLE}</h2>
      <div className="ccc-module-tools">
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label={`重新读取 ${TITLE}`} disabled={busy}
          onClick={() => { setDraft(null); reread(); }} />
      </div>
    </div>
    {!loaded ? <Loading label="正在读取电源计划…" /> : <div className="ccc-power-setting">
      <div>
        <label htmlFor="windows-power-plan">{PICKER}</label>
      </div>
      <div className="ccc-power-setting-picker">
        <Select id="windows-power-plan" label={PICKER} size="compact" value={target?.id ?? current?.id ?? ""}
          options={plans.map(plan => ({ value: plan.id, label: label(plan), description: plan.id === current?.id ? "当前启用" : undefined }))}
          placeholder="计划不可用" disabled={locked || plans.length < 2}
          onChange={id => setDraft(profile && id !== current?.id ? { revision: profile.revision, id } : null)} />
      </div>
    </div>}
    {profile?.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
    {profile && !profile.blockedReason && plans.length === 1 && <p className="ccc-control-caption">Windows 目前只提供一个电源计划。</p>}
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!error && profile && draft === "stale" && <p className="ccc-control-caption" role="status">电源计划已变化，请重新选择。</p>}
    {profile && target && <ConfigurationAction label={TITLE} applyLabel="应用电源计划" revision={profile.revision}
      disabled={locked} onConfirm={() => apply(target.id)}>
      <FactList compact facts={[{ label: "待应用", value: label(target) }]} />
    </ConfigurationAction>}
  </Surface>;
}
