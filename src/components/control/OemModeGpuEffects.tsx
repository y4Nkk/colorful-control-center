import type { OemModeGpuReview } from "../../api/client";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { GpuModeRawStateFacts, GpuModeDriverRanges, GpuModeTargetFacts } from "./GpuModeFacts";
import { GpuPowerVisual } from "./GpuPowerVisual";
import { GpuNodeStages } from "./GpuNodeStages";
import { gpuModeError, gpuSkipReason, gpuTarget } from "./gpu-values";

export function OemModeGpuEffects({ review }: { review: OemModeGpuReview }) {
  const error = gpuModeError(review);
  if (review.status === "unavailable") return <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />GPU联动：{error}</p>;
  const { node, nodeSupported, action, clocks, onAc, stages } = review.value;
  const changesNode = action && node && action.target.enabled !== node.enabled;
  const decision = clocks.candidate.decision;
  return <section className="ccc-mode-gpu-columns" aria-label="模式GPU联动预览">
    <Surface variant="inset" className="ccc-mode-gpu-part">
      <GpuPowerVisual label="当前GPU" enabled={node?.enabled ?? null} automatic={node?.automaticPowerOff ?? null} />
      <FactList compact layout="tiles" facts={[
        { label: "供电", value: onAc === null ? "未知" : onAc ? "插电" : "电池" },
      ]} />
      {clocks.driver && <GpuModeRawStateFacts value={clocks.driver.current} />}
      {clocks.deferredReason && <span className="ccc-control-caption">开启独显后再读取频率</span>}
      {decision?.skipped && <span className="ccc-control-caption">本次不调频</span>}
      {clocks.driver && <GpuModeDriverRanges driver={clocks.driver} />}
    </Surface>
    <Surface variant="inset" className="ccc-mode-gpu-part">
      <strong>切换后的 GPU 目标</strong>
      <GpuNodeStages stages={stages} action={action} current={node} />
      {decision?.target && (clocks.candidate.projected ? <GpuModeRawStateFacts value={clocks.candidate.projected} /> : <GpuModeTargetFacts target={decision.target} />)}
      <section className="ccc-mode-clock-facts" aria-label="联动详情">
        <p>{changesNode ? `${action.stage === "beforeMode" ? "切换模式前" : "调整频率和节能设置后"}${action.target.enabled ? "开启" : "关闭"}独显电源` : nodeSupported ? "保持独显电源现状" : "本次不改动独显电源"}</p>
        {action && <p>「自动关断」设置保持不变。</p>}
        {decision?.target && <p>{gpuTarget(decision.target)}</p>}
        {clocks.deferredReason && <p className="ccc-control-caption">{clocks.deferredReason}</p>}
        {decision?.skipped && <p className="ccc-control-caption">{gpuSkipReason(decision.skipped)}。</p>}
      </section>
      {error && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{error}</p>}
    </Surface>
  </section>;
}
