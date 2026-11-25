import { OEM_POWER_MODES, PERFORMANCE_OPTION_LABELS, fanModeLabel, type OemModeTarget } from "../../api/client";
import { FactList } from "../FactList";
import { ASPM_NAMES, PLAN_NAMES } from "./power-plan-values";
import { energyValue } from "./energy-values";
import { OemModeGpuEffects } from "./OemModeGpuEffects";

export function OemModeEffects({ target }: { target: OemModeTarget }) {
  const { fan } = target;
  const fanMode = fan.control === "firmware" ? "由固件管理" : fanModeLabel(fan.mode);
  const offset = fan.control === "explicit" ? fan.offsetPct : null;
  const performanceOptions = [target.turbo && PERFORMANCE_OPTION_LABELS.turboFan, target.dtt && PERFORMANCE_OPTION_LABELS.dtt].filter(Boolean).join("、");
  return <>
    <FactList compact layout="tiles" label="模式联动目标" facts={[
      { label: "插电模式偏好", value: target.acMode === null ? "保持当前" : OEM_POWER_MODES[target.acMode] ?? target.acMode },
      { label: "节能策略", value: target.energy === null ? "保持当前" : energyValue(target.energy) },
      { label: "风扇", value: offset === null ? fanMode : `${fanMode} · 加速偏移 ${offset}%` },
      ...(performanceOptions ? [{ label: "性能选项", value: performanceOptions }] : []),
      ...(target.aspm ? [{ label: "链路节能", value: `插电 ${ASPM_NAMES[target.aspm[0]]} / 电池 ${ASPM_NAMES[target.aspm[1]]}` }] : []),
      ...(target.powerPlan != null ? [{ label: "电源计划", value: PLAN_NAMES[target.powerPlan] }] : []),
    ]} />
    {target.gpu && <OemModeGpuEffects review={target.gpu} />}
  </>;
}
