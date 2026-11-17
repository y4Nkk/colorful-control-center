import { restoreBatteryThresholds, type BatteryThresholdPlan, type BatteryThresholdProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { Collapsible } from "../Collapsible";
import { MetricReadout } from "./MetricReadout";
import { powerPlanName } from "./power-plan-values";

type Props = { profile: BatteryThresholdProfile; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
const names = ["低电量", "严重电量", "预留电量"];
const targets = [25, 20, 22];
const values = (plan: BatteryThresholdPlan) => plan.values.map((value, i) => `${names[i]} ${value[0]} / ${value[1]}%`).join(" · ");

export function BatteryThresholdFacts({ profile, busy, run }: Props) {
  const { current, recovery } = profile;
  return <section className="ccc-battery-threshold-facts" aria-label="原厂低电量阈值事实与预览">
    <strong>原厂低电量阈值</strong>
    <div className="ccc-battery-threshold-columns">
      <Surface variant="inset" className="ccc-battery-threshold-part">
        <strong>当前计划 · 插电 / 电池</strong>
        {current ? <>
          {current.plans.map(plan => <div key={plan.id} className="ccc-battery-threshold-plan">
            <span>{powerPlanName(plan.id)}{plan.id === current.active ? " · 当前活动" : ""}</span>
            <span className="ccc-control-caption">{values(plan)}</span>
          </div>)}
          <p className="ccc-control-caption">原厂缓存：{current.cache.map(value => `${value}%`).join(" / ")}</p>
        </> : <p className="ccc-control-caption">计划阈值当前不可读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-threshold-part">
        <strong>事件目标</strong>
        <div className="ccc-battery-threshold-targets">{targets.map((target, index) =>
          <MetricReadout key={names[index]} label={names[index]} value={target} maximum={100} unit="%" colorVar="var(--ccc-action)" />)}</div>
        <Collapsible summary="阈值作用范围"><p>接管后对活动计划和存在的标准计划分别设置插电与电池阈值；不创建缺失计划。</p></Collapsible>
      </Surface>
    </div>
    {(recovery || profile.restoreError) && <Surface variant="inset" className="ccc-battery-threshold-part">
      <strong>初始恢复目标</strong>
      {recovery && <>
        <div className="ccc-battery-threshold-recovery-plans">{recovery.baseline.plans.map((plan, index) => <div key={plan.id} className="ccc-battery-threshold-plan">
          <span>{powerPlanName(plan.id)}</span>
          <span className="ccc-control-caption">{plan.values.map((pair, level) => `${names[level]} ${pair.map((value, supply) => recovery.owned[index][level][supply] ? `${recovery.current[index].values[level][supply]}→${value}%` : "保留").join(" / ")}`).join(" · ")}</span>
        </div>)}</div>
        <p className="ccc-control-caption">恢复只修改本应用接管字段及缓存，保留其他计划和字段。</p>
      </>}
      {profile.restoreError && <p className="ccc-control-error">{profile.restoreError}</p>}
      {recovery && <ConfirmButton icon="undo" key={profile.restoreRevision ?? "battery-restore-unavailable"} variant="secondary"
        label="恢复初始电池阈值" confirmLabel="确认停止监听并恢复上述阈值？" busy={busy} disabled={!profile.restoreRevision}
        onConfirm={() => run(() => restoreBatteryThresholds(profile.restoreRevision!), "初始电池阈值已恢复", false)} />}
    </Surface>}
  </section>;
}
