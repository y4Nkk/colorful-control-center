import { Icon } from "../Icon";
import { ENERGY_POLICY_LABEL } from "../../api/battery";
import type { ProfileSettings } from "../../api/profiles";
import { ENERGY_DAYS, energyHour } from "../../lib/energy-policy";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { MetricReadout } from "../control/MetricReadout";
import { EnergyTimeline } from "../control/EnergyTimeline";
import { DetailsDialog } from "../DetailsDialog";

export function ProfileBatteryPreview({ settings }: { settings: ProfileSettings }) {
  const charge = settings.batteryCharge, energy = settings.batteryEnergy;
  const policy = energy?.policy;
  return <>
    {charge && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案充电保护">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="charging" />充电保护</h3><strong>{charge.values.enabled ? "启用阈值控制" : "关闭阈值控制"}</strong>
        <DetailsDialog title="充电策略应用顺序"><p className="ccc-control-caption">应用时核对固件候选，保存原始阈值。</p>
        <p className="ccc-control-caption">{energy ? "先停止节能计划并恢复其充电策略，再应用这些阈值。" : "节能计划正在管理阈值时不能单独应用。"}</p></DetailsDialog>
      </div>
      {charge.values.enabled ? <div className="ccc-profile-thresholds">
        <MetricReadout label="开始充电阈值" value={charge.values.start} maximum={100} unit="%" colorVar="var(--ccc-action)" />
        <MetricReadout label="停止充电阈值" value={charge.values.stop} maximum={100} unit="%" colorVar="var(--ccc-safe)" />
      </div> : <FactList compact facts={[{ label: "原始开始阈值", value: `${charge.values.start}%` }, { label: "原始停止阈值", value: `${charge.values.stop}%` }]} />}
    </Surface>}
    {energy && policy && <Surface as="section" variant="group" className="ccc-profile-device" aria-label={`档案${ENERGY_POLICY_LABEL}`}>
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="sleep" />{ENERGY_POLICY_LABEL}</h3><strong>{policy.enabled ? "启用节能时段" : "停止节能时段"}</strong>
        <p className="ccc-control-caption">{policy.everyDay ? "每天" : ENERGY_DAYS.filter((_, index) => policy.weekdays & (1 << index)).map(day => `周${day}`).join("、") || "未选择星期"}</p>
        <p className="ccc-control-caption">{policy.automatic ? energy.capability.factoryDefaulted ? "原厂默认" : "原厂自动" : "自定义"} · {policy.limits.floor}–{policy.limits.ceiling}%</p>
        <DetailsDialog title="节能策略说明"><p>{policy.enabled ? "计划管理充电阈值，需保持固件能力一致。" : "先停止并清空固件时段，恢复启用前充电策略。"}</p></DetailsDialog>
      </div>
      <div className="ccc-profile-values"><span className="ccc-control-caption">保存时段{policy.enabled ? "" : "（计划关闭）"}</span>
        <EnergyTimeline slots={policy.slots} label="档案节能时段" />
        <FactList compact facts={policy.slots.map((slot, index) => ({ label: `时段 ${index + 1}`,
          value: slot ? `${energyHour(slot.start)}–${energyHour(slot.end)}` : "未配置" }))} />
      </div>
    </Surface>}
  </>;
}
