import { OEM_POWER_MODES, type SupplyModeStatus } from "../../api/client";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { fmtClock } from "./format";
import { GpuModeRawStateFacts, GpuModeDriverRanges } from "./GpuModeFacts";
import { energyValue } from "./energy-values";
import { PowerSourceVisual } from "./PowerSourceVisual";

export function SupplyModeCurrent({ view }: { view: SupplyModeStatus }) {
  const facts = view.review, latest = view.last, clocks = facts?.clocks;
  return <div className="ccc-supply-mode-current">
    <Surface variant="inset" className="ccc-supply-mode-summary">
      <h3>当前状态</h3>
      <PowerSourceVisual source={facts?.onAc === true ? "ac" : facts?.onAc === false ? "battery" : "unknown"} />
      <FactList compact facts={[
        { label: "当前模式", value: facts ? OEM_POWER_MODES[facts.currentMode] ?? `未知模式 ${facts.currentMode}` : "未知" },
        { label: "交流偏好", value: facts ? OEM_POWER_MODES[facts.acMode] ?? `未知模式 ${facts.acMode}` : "未知" },
        { label: "当前固件节能", value: facts ? energyValue(facts.currentEnergy) : "未知" },
        { label: "独显状态", value: facts?.gpu.current ? facts.gpu.current.enabled ? "开启" : "关闭" : "未报告" },
        { label: "自动关断", value: facts?.gpu.current ? facts.gpu.current.automaticPowerOff ? "允许" : "关闭" : "未报告" },
      ]} />
      {facts?.softwareFanActive && <p className="ccc-control-caption">应用模式前会退出软件风扇曲线；初始恢复包含原曲线。</p>}
    </Surface>
    <Surface as="section" variant="inset" className="ccc-supply-mode-summary" aria-label="当前GPU频率">
      <h3>GPU频率实读</h3><GpuModeRawStateFacts value={clocks?.driver?.current ?? null} />
      {clocks?.deferredReason && <p className="ccc-control-caption">{clocks.deferredReason}</p>}
      {clocks?.driverError && <p className="ccc-control-error" role="alert">{clocks.driverError}</p>}
      {clocks?.driver && <GpuModeDriverRanges driver={clocks.driver} />}
    </Surface>
    {latest && <Surface as="section" variant="inset" className="ccc-supply-mode-summary" aria-label="最近原厂自动切换">
      <h3>最近自动结果</h3><strong>{latest.applied ? latest.error ? "已应用，收尾待处理" : "已应用" : latest.error ? "未完成" : "本次无模式目标"}</strong>
      <span className="ccc-control-caption">{fmtClock(latest.atMs)} · {latest.trigger === "supplyChange" ? "供电变化" : latest.trigger === "resume" ? "唤醒规则" : "启动规则"} · {latest.onAc === true ? "插电" : latest.onAc === false ? "电池供电" : "供电未报告"}</span>
      {latest.mode !== null && <p>{latest.previousMode === null ? "未知模式" : OEM_POWER_MODES[latest.previousMode]} → {OEM_POWER_MODES[latest.mode]}</p>}
      {latest.error && <p className="ccc-control-error" role="alert">{latest.error}</p>}
    </Surface>}
  </div>;
}
