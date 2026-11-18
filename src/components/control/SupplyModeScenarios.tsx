import { OEM_POWER_MODES, type SupplyConditions, type OemModeGpuAction, type SupplyModeReview } from "../../api/client";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { ModeArtwork } from "./ModeArtwork";
import { OemModeEffects } from "./OemModeEffects";
import { gpuSkipReason, gpuTarget } from "./gpu-values";

const CONDITIONS: { key: keyof SupplyConditions<unknown>; label: string }[] = [
  { key: "ac", label: "接电时" }, { key: "battery", label: "使用电池时" },
  { key: "startupAc", label: "启动 · 插电" }, { key: "startupBattery", label: "启动 · 电池" },
  { key: "resumeAc", label: "唤醒 · 插电" }, { key: "resumeBattery", label: "唤醒 · 电池" },
];
function powerAction(action: OemModeGpuAction | null) {
  if (!action) return "独显与自动关断保持";
  return `${action.stage === "beforeMode" ? "切模式前" : "切模式后"}确保独显${action.target.enabled ? "开启" : "关闭"} · 自动关断${action.target.automaticPowerOff ? "允许" : "关闭"}`;
}
export function SupplyModeScenarios({ review }: { review: SupplyModeReview }) {
  return <div className="ccc-supply-mode-scenarios" aria-label="六种条件的原厂联动">
    {CONDITIONS.map(({ key, label }) => <Scenario key={key} review={review} condition={key} label={label} />)}
  </div>;
}
function Scenario({ review, condition, label }: { review: SupplyModeReview; condition: keyof SupplyConditions<unknown>; label: string }) {
  const target = review[condition], gpu = review.gpu[condition], clocks = review.clocks?.[condition];
  return <Surface as="section" variant="inset" className="ccc-supply-mode-case" aria-label={label}>
    <h3 className="ccc-rule-heading"><Icon name={condition.toLowerCase().includes("battery") ? "battery" : "plug"} />{label}</h3>
    {target ? <>
      <div className="ccc-rule-mode"><Icon name="arrow-right" /><ModeArtwork mode={target.mode} /><strong>{OEM_POWER_MODES[target.mode] ?? `未知模式 ${target.mode}`}</strong></div>
      {gpu.action && <span className="ccc-chip">独显目标 · {gpu.action.target.enabled ? "开启" : "关闭"}</span>}
      {[...new Set([target.unavailableReason, target.gpu?.status === "unavailable" ? target.gpu.value : null, gpu.unavailableReason, clocks?.unavailableReason].filter(Boolean))].map((reason, i) => <p className="ccc-control-error" key={i} role="alert">{reason}</p>)}
      <DetailsDialog title="联动目标">
      <OemModeEffects target={target} />
      {target.powerPlan === null && <p className="ccc-control-caption">保持当前Windows电源计划</p>}
      <p className="ccc-control-caption">{review.gpu.supported ? powerAction(gpu.action) : "无独显电源联动"}</p>
      {clocks?.decision?.target && <p className="ccc-supply-mode-clock">{gpuTarget(clocks.decision.target)}</p>}
      {clocks?.decision?.skipped && <p className="ccc-control-caption">{gpuSkipReason(clocks.decision.skipped)}</p>}
      {!clocks && <p className="ccc-control-caption">{condition.startsWith("resume") ? gpuSkipReason("resume") : "本次未读取GPU调频策略"}</p>}
      </DetailsDialog>
    </> : <p className="ccc-control-caption">原厂此条件不切换模式</p>}
  </Surface>;
}
