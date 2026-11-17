import { FAN_MODES, OEM_POWER_MODES, type OemEnergyState, type OemModeGpuState, type SupplyModeRecovery as Recovery } from "../../api/client";
import { Collapsible } from "../Collapsible";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { GpuModeRawStateFacts, GpuModeObservationFacts } from "./GpuModeFacts";
import { ASPM_NAMES, powerPlanName } from "./power-plan-values";
import { energyValue } from "./energy-values";

const onOff = (value: boolean) => value ? "开启" : "关闭";
const fanMode = (value: number) => FAN_MODES.find(mode => mode.id === value)?.label ?? `未知模式 ${value}`;
const modeName = (value: number) => OEM_POWER_MODES[value] ?? `未知模式 ${value}`;
const aspm = (value: number) => ASPM_NAMES[value] ?? `未知值 ${value}`;
function system(value: Recovery["currentSystem"]) {
  if (!value) return "未接管";
  return `${powerPlanName(value[0])}${value[1] ? ` · 插电${aspm(value[1][0])}／电池${aspm(value[1][1])}` : " · ASPM未知"}`;
}
function energy(value: OemEnergyState | null) {
  return value === null ? "未接管" : energyValue(value);
}
function gpuPower(value: OemModeGpuState | null) {
  return !value ? "未接管" : `${value.name} · ${onOff(value.target.enabled)} · 自动关断${value.target.automaticPowerOff ? "允许" : "关闭"}`;
}
export function SupplyModeRecovery({ value }: { value: Recovery }) {
  const origin = value.initialGpuClocks?.origin;
  return <div className="ccc-supply-mode-recovery" aria-label="原厂模式初始恢复预览">
    <FactList compact facts={[
      { label: "电源模式", value: `${modeName(value.currentMode)} → ${modeName(value.initialMode)}` },
      ...(value.initialAcMode === null ? [] : [{ label: "插电模式偏好", value: `${value.currentAcMode === null ? "未知" : modeName(value.currentAcMode)} → ${modeName(value.initialAcMode)}` }]),
      { label: "Turbo风扇偏好", value: `${onOff(value.currentTurbo)} → ${onOff(value.initialTurbo)}` },
      { label: "DTT偏好", value: `${onOff(value.currentDtt)} → ${onOff(value.initialDtt)}` },
      { label: "风扇模式", value: `${fanMode(value.currentFanMode)} → ${fanMode(value.initialFanMode)}` },
      { label: "风扇加速偏移", value: `${value.currentFanOffset}% → ${value.initialFanOffset}%` },
      { label: "软件曲线", value: `${onOff(value.currentSoftwareFan)} → ${onOff(value.initialSoftwareFan)}` },
      { label: "保存的风扇参数", value: value.fanSavedSettingsChanged ? "恢复接管前的参数" : "与初始相同" },
      { label: "活动电源计划", value: `${system(value.currentSystem)} → ${system(value.initialSystem)}` },
      { label: "固件节能", value: `${energy(value.currentEnergy)} → ${energy(value.initialEnergy)}` },
      { label: "独显与自动关断", value: `${gpuPower(value.currentGpuPower)} → ${gpuPower(value.initialGpuPower)}` },
    ]} />
    {value.planChanges.map(plan => <Surface key={plan.scheme} variant="inset" className="ccc-supply-mode-summary">
      <h3>{powerPlanName(plan.scheme)} · PCIe节能恢复</h3>
      <p>插电：{aspm(plan.current[0])} → {aspm(plan.initial[0])}</p>
      <p>电池：{aspm(plan.current[1])} → {aspm(plan.initial[1])}</p>
    </Surface>)}
    {value.initialGpuClocks && <Surface variant="inset" className="ccc-supply-mode-summary">
      <h3>GPU频率恢复</h3>
      <p className="ccc-control-caption">{origin === "afterEnable" ? "初始频率在本次开启独显后取得，恢复后仍按初始独显状态处理。" : origin === "poweredOff" ? "初始频率在独显节点关闭时，经本次授权的控制会话实读取得。" : "恢复到首次模式写入前实读的频率。"}</p>
      <div className="ccc-supply-mode-pair">
        <div className="ccc-supply-mode-stack"><h4>当前观察</h4><GpuModeObservationFacts value={value.currentGpuClocks} /></div>
        <div className="ccc-supply-mode-stack"><h4>初始目标</h4><GpuModeRawStateFacts value={value.initialGpuClocks.state} /></div>
      </div>
    </Surface>}
    <Collapsible summary="查看完整风扇恢复参数">
      <div className="ccc-supply-mode-pair">
        <div className="ccc-supply-mode-stack"><h4>当前参数</h4><pre className="ccc-supply-mode-raw ccc-mono">{JSON.stringify(value.currentFan, null, 2)}</pre></div>
        <div className="ccc-supply-mode-stack"><h4>初始参数</h4><pre className="ccc-supply-mode-raw ccc-mono">{JSON.stringify(value.initialFan, null, 2)}</pre></div>
      </div>
    </Collapsible>
  </div>;
}
