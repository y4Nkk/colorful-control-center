import type { GpuPowerDevice, GpuPowerStages, OemModeGpuAction } from "../../api/client";
import { Icon } from "../Icon";
import { Surface } from "../Surface";

export function GpuNodeStages({ stages, action, current }: { stages: GpuPowerStages | null; action: OemModeGpuAction | null; current: GpuPowerDevice | null }) {
  const steps = stages ? [
    { phase: "准备", request: stages.preparation }, { phase: "切换前", request: stages.beforeMode },
    { phase: "切换后", request: stages.afterMode }, { phase: "最终", request: stages.finish },
  ].flatMap(step => step.request ? [{ phase: step.phase, target: step.request.target,
    changesAutomatic: step.request.expected.automaticPowerOff !== step.request.target.automaticPowerOff }] : [])
    : action && current && (action.target.enabled !== current.enabled || action.target.automaticPowerOff !== current.automaticPowerOff)
      ? [{ phase: action.stage === "beforeMode" ? "切换前" : "切换后", target: action.target,
        changesAutomatic: action.target.automaticPowerOff !== current.automaticPowerOff }] : [];
  if (!steps.length) return <span className="ccc-control-caption">电源保持</span>;
  return <ol className="ccc-gpu-node-stages" aria-label="独显联动步骤">
    {steps.map((step, index) => <li key={`${step.phase}-${index}`}>
      {index > 0 && <Icon name="chevron-right" size={16} />}
      <Surface variant="card" className="ccc-gpu-node-step" data-enabled={String(step.target.enabled)}>
        <span className="ccc-control-caption">{step.phase}</span><strong>{step.target.enabled ? "开启" : "关闭"}</strong>
        {step.changesAutomatic && <span className="ccc-control-caption">自动关断{step.target.automaticPowerOff ? "开" : "关"}</span>}
      </Surface>
    </li>)}
  </ol>;
}
