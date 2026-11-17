import type { ProfileModeApproval } from "../../api/client";
import { Surface } from "../Surface";
import { Icon } from "../Icon";
import { Collapsible } from "../Collapsible";
import { GpuModeSupplyTargets, GpuModeDriverRanges } from "../control/GpuModeFacts";
import { GpuPowerVisual } from "../control/GpuPowerVisual";

export function ProfileModeAuthorization({ approval }: { approval: ProfileModeApproval | null }) {
  if (!approval) return null;
  const gpu = approval.target.gpu?.status === "ready" ? approval.target.gpu.value : null;
  const rules = approval.nodeRules.filter(rule => approval.allowedAutomaticPowerOff?.includes(rule.automaticPowerOff));
  return <Surface as="section" variant="group" className="ccc-profile-mode-authorization" aria-label="后台模式GPU联动">
    <div className="ccc-panel-head"><strong className="ccc-icon-title"><Icon name="confirm" />自动使用时的 GPU 联动</strong>
      <span className="ccc-chip">{approval.clocks.deferredReason ? "开启后核对驱动" : approval.clocks.driver ? "驱动已核对" : "本模式不调频"}</span></div>
    <div className="ccc-mode-gpu-columns">
      <Surface variant="inset" className="ccc-mode-gpu-part"><strong>供电目标</strong><GpuModeSupplyTargets choices={approval.clocks.choices} /></Surface>
      <Surface variant="inset" className="ccc-mode-gpu-part">
        <GpuPowerVisual label="控制节点" enabled={gpu?.node?.enabled ?? null} automatic={gpu?.node?.automaticPowerOff ?? null} />
        {rules.length ? <ul className="ccc-profile-mode-rules" aria-label="允许的独显模式条件">{rules.map(rule => <li key={String(rule.automaticPowerOff)}>
          <Icon name={rule.action?.target.enabled === false ? "shutdown" : "gpu"} size={16} />
          <span>自动关断{rule.automaticPowerOff ? "开" : "关"}</span>
          <strong>{rule.action ? `${rule.action.stage === "beforeMode" ? "模式前" : "模式后"}${rule.action.target.enabled ? "开启" : "关闭"}` : "保持"}</strong>
        </li>)}</ul> : <span className="ccc-control-caption">无节点控制</span>}
      </Surface>
    </div>
    {approval.clocks.driver && <GpuModeDriverRanges driver={approval.clocks.driver} />}
    <Collapsible summary="执行与恢复说明"><p>每次执行重新检查设备与目标；开启后的驱动身份在写入前记录。失败恢复本次前态，已有初始恢复状态保留。</p></Collapsible>
  </Surface>;
}
