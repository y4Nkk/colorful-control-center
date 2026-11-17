import { FAN_MODES, OEM_POWER_MODES, type OemModeTarget } from "../../api/client";

import { ASPM_NAMES, PLAN_NAMES } from "./power-plan-values";
import { energyValue } from "./energy-values";
import { OemModeGpuEffects } from "./OemModeGpuEffects";

export function OemModeEffects({ target }: { target: OemModeTarget }) {
  return <><div className="ccc-mode-effects" aria-label="模式联动目标">
    <span className="ccc-chip">{target.acMode === null ? "保留插电偏好" : `插电偏好记为${OEM_POWER_MODES[target.acMode] ?? target.acMode}`}</span>
    <span className="ccc-chip">{target.energy === null ? "保持固件节能状态" : energyValue(target.energy)}</span>
    <span className="ccc-chip">风扇 {FAN_MODES.find(mode => mode.id === target.fanMode)?.label ?? target.fanMode}</span>
    <span className="ccc-chip">加速偏移 {target.fanOffsetPct}%</span>
    {target.turbo && <span className="ccc-chip">性能增强</span>}
    {target.dtt && <span className="ccc-chip">动态调优</span>}
    {target.aspm && <span className="ccc-chip">链路节能：插电 {ASPM_NAMES[target.aspm[0]]} / 电池 {ASPM_NAMES[target.aspm[1]]}</span>}
    {target.powerPlan != null && <span className="ccc-chip">电源计划 {PLAN_NAMES[target.powerPlan]}</span>}
  </div>{target.gpu && <OemModeGpuEffects review={target.gpu} />}</>;
}
