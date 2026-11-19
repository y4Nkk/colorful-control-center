import { FAN_MODES, NOISELESS_FAN_LABEL, OEM_POWER_MODES, PERFORMANCE_OPTION_LABELS, type OemEnergyState, type OemModeRestoreTarget } from "../../api/client";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { ASPM_NAMES, powerPlanName } from "./power-plan-values";
import { energyValue } from "./energy-values";
import { OemModeGpuRestore } from "./OemModeGpuRestore";

const modeName = (mode: number) => OEM_POWER_MODES[mode] ?? `未知模式 ${mode}`;
export function OemModeRestore({ target, currentMode, currentAcMode, currentEnergy, blockedReason, busy, disabled, onRestore }: {
  target: OemModeRestoreTarget; currentMode: number; currentAcMode: number | null;
  currentEnergy: OemEnergyState | null;
  blockedReason: string | null;
  busy: boolean; disabled: boolean; onRestore: (revision: string) => Promise<void>;
}) {
  const preview = <Surface as="section" variant="plain" className="ccc-mode-restore" aria-label="初始模式恢复目标">
      <div className="ccc-mode-restore-columns">
      <section className="ccc-mode-restore-block" aria-label="模式与性能恢复目标"><h3>模式与性能</h3>
      <FactList compact layout="tiles" facts={[
        { label: "模式", value: `${modeName(currentMode)} → ${modeName(target.mode)}` },
        { label: "插电模式偏好", value: target.acMode === null ? "保持当前偏好" : `${currentAcMode === null ? "未知" : modeName(currentAcMode)} → ${modeName(target.acMode)}` },
        { label: `${PERFORMANCE_OPTION_LABELS.turboFan}偏好`, value: target.turbo ? "开启" : "关闭" },
        { label: `${PERFORMANCE_OPTION_LABELS.dtt} 偏好`, value: target.dtt ? "开启" : "关闭" },
        ...(target.entertainmentQuiet === null ? [] : [{ label: `娱乐 · ${NOISELESS_FAN_LABEL}`, value: target.entertainmentQuiet ? "开启" : "关闭" }]),
        ...(target.energy === null ? [] : [{ label: "固件节能", value: `${currentEnergy ? energyValue(currentEnergy) : "当前未知"} → ${energyValue(target.energy)}` }]),
      ]} /></section>
      <section className="ccc-mode-restore-block" aria-label="风扇恢复目标"><h3>散热策略</h3>
      <FactList compact layout="tiles" facts={[
        { label: "风扇模式", value: FAN_MODES.find(mode => mode.id === target.fan.mode)?.label ?? `模式 ${target.fan.mode}` },
        { label: "风扇加速偏移", value: `${target.fan.offset}%` },
        { label: "软件曲线", value: target.fan.software ? "恢复初始曲线" : "关闭" },
      ]} /></section>
      {target.system && <section className="ccc-mode-restore-block" aria-label="系统计划恢复目标"><h3>Windows计划</h3>
      <FactList compact layout="tiles" facts={[
        { label: "活动电源计划", value: powerPlanName(target.system[0]) },
      ]} /></section>}
      {target.systemPlans.map(plan => <section key={plan.scheme} className="ccc-mode-restore-block" aria-label={`${powerPlanName(plan.scheme)}链路节能恢复`}>
        <h3>{powerPlanName(plan.scheme)}</h3>
        <FactList compact layout="tiles" facts={[true, false].map((ac, index) => ({ label: ac ? "插电PCIe节能" : "电池PCIe节能",
          value: `${plan.current ? ASPM_NAMES[plan.current[index]] : "未知"} → ${ASPM_NAMES[plan.initial[index]]}` }))} />
      </section>)}
      </div>
      <OemModeGpuRestore target={target} />
      <DetailsDialog title="查看完整风扇恢复参数"><pre className="ccc-mode-restore-raw ccc-mono">{JSON.stringify(target.fan, null, 2)}</pre></DetailsDialog>
    </Surface>;
  return <>
    {blockedReason && <DetailsDialog title="模式恢复暂不可用" wide>
      <p>{blockedReason}</p>{preview}
    </DetailsDialog>}
    <ConfigurationAction label="模式与性能选项" revision={target.revision} restore disabled={busy || disabled}
      onConfirm={() => onRestore(target.revision)}>
      {preview}
    </ConfigurationAction>
  </>;
}
