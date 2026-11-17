import { OEM_POWER_MODES, type OemModeCycleProfile } from "../../api/client";
import { Collapsible } from "../Collapsible";
import { Surface } from "../Surface";
import { GpuModeDriverRanges, GpuModeRawStateFacts, GpuModeSupplyTargets } from "./GpuModeFacts";
import { OemModeEffects } from "./OemModeEffects";
import { GpuPowerVisual } from "./GpuPowerVisual";
import { ModeArtwork } from "./ModeArtwork";

export function ModeCycleFacts({ profile }: { profile: OemModeCycleProfile }) {
  const { clocks, node } = profile;
  return <Collapsible summary="电源模式循环联动预览">
    <section className="ccc-mode-gpu-columns" aria-label="电源模式循环GPU预览">
      <Surface variant="inset" className="ccc-mode-gpu-part">
        <GpuPowerVisual label="当前GPU" enabled={node?.enabled ?? null} automatic={node?.automaticPowerOff ?? null} />
        {clocks.driver && <><GpuModeRawStateFacts value={clocks.driver.current} /><GpuModeDriverRanges driver={clocks.driver} /></>}
        {clocks.deferredReason && <span className="ccc-chip">开启后核对频率</span>}
        {clocks.driverError && <p className="ccc-control-error" role="alert">{clocks.driverError}</p>}
        {!clocks.driver && !clocks.deferredReason && !clocks.driverError && <span className="ccc-chip">当前频率未读取</span>}
        <Collapsible summary="设备与恢复说明">
          <p>{node?.name ?? (profile.nodeSupported ? "独显节点未报告" : "固件未声明独显节点控制")}</p>
          {clocks.deferredReason && <p>{clocks.deferredReason}</p>}
          <p>设备和驱动范围变化需重新确认；初始状态在预设模式区域恢复。</p>
        </Collapsible>
      </Surface>
      <Surface variant="inset" className="ccc-mode-gpu-part">
        <strong>循环候选与供电目标</strong>
        <div className="ccc-mode-cycle-options">{profile.modes.map(mode => {
          const nodeChoice = profile.nodeChoices.find(choice => choice.mode === mode.mode);
          const action = nodeChoice?.action;
          return <Surface variant="card" className="ccc-mode-clock-facts ccc-mode-cycle-choice" key={mode.mode} role="region" aria-label={`${OEM_POWER_MODES[mode.mode]}循环目标`}>
            <div className="ccc-mode-cycle-heading"><span className="ccc-mode-cycle-art"><ModeArtwork mode={mode.mode} /></span><strong>{OEM_POWER_MODES[mode.mode]}</strong>
              <span className="ccc-chip">{action ? `${action.stage === "beforeMode" ? "切换前" : "切换后"} · 确保${action.target.enabled ? "开启" : "关闭"}` : "电源保持"}</span>
            </div>
            {nodeChoice?.unavailableReason && <p className="ccc-control-error" role="alert">{nodeChoice.unavailableReason}</p>}
            <GpuModeSupplyTargets choices={clocks.choices.filter(choice => choice.mode === mode.mode)} />
            <Collapsible summary="风扇与系统联动"><OemModeEffects target={mode} /></Collapsible>
          </Surface>;
        })}</div>
      </Surface>
    </section>
  </Collapsible>;
}
