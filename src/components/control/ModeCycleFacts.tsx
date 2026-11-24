import { OEM_POWER_MODES, type OemModeCycleProfile } from "../../api/client";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { GpuModeDriverRanges, GpuModeRawStateFacts, GpuModeSupplyTargets } from "./GpuModeFacts";
import { OemModeEffects } from "./OemModeEffects";
import { GpuPowerVisual } from "./GpuPowerVisual";
import { ModeArtwork } from "./ModeArtwork";

export function ModeCycleFacts({ profile }: { profile: OemModeCycleProfile }) {
  const { clocks, node } = profile;
  return <DetailsDialog title="电源模式循环会改动什么">
    <section className="ccc-mode-gpu-columns" aria-label="电源模式循环GPU预览">
      <Surface variant="inset" className="ccc-mode-gpu-part">
        <GpuPowerVisual label="当前GPU" enabled={node?.enabled ?? null} automatic={node?.automaticPowerOff ?? null} />
        {clocks.driver && <><GpuModeRawStateFacts value={clocks.driver.current} /><GpuModeDriverRanges driver={clocks.driver} /></>}
        {clocks.deferredReason && <span className="ccc-control-caption">开启独显后再读取频率</span>}
        {clocks.driverError && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{clocks.driverError}</p>}
        {!clocks.driver && !clocks.deferredReason && !clocks.driverError && <span className="ccc-control-caption">当前频率未读取</span>}
        <section className="ccc-mode-clock-facts" aria-label="设备与恢复说明">
          <p className="ccc-control-caption">{node?.name ?? (profile.nodeSupported ? "未检测到独显" : "这台电脑不支持控制独显电源")}</p>
          {clocks.deferredReason && <p className="ccc-control-caption">{clocks.deferredReason}</p>}
          <p>显卡设备或驱动支持的频率范围发生变化后，需要重新确认接管。</p>
        </section>
      </Surface>
      <Surface variant="inset" className="ccc-mode-gpu-part">
        <strong>会切换到的模式和 GPU 目标</strong>
        <div className="ccc-mode-cycle-options">{profile.modes.map(mode => {
          const nodeChoice = profile.nodeChoices.find(choice => choice.mode === mode.mode);
          const action = nodeChoice?.action;
          return <Surface variant="card" className="ccc-mode-clock-facts ccc-mode-cycle-choice" key={mode.mode} role="region" aria-label={`${OEM_POWER_MODES[mode.mode]}循环目标`}>
            <div className="ccc-mode-cycle-heading"><span className="ccc-mode-cycle-art"><ModeArtwork mode={mode.mode} /></span><strong>{OEM_POWER_MODES[mode.mode]}</strong>
              <span className="ccc-control-caption">{action ? `${action.stage === "beforeMode" ? "切换前" : "切换后"} · 确保${action.target.enabled ? "开启" : "关闭"}` : "不改动独显电源"}</span>
            </div>
            {nodeChoice?.unavailableReason && <p className="ccc-control-error" role="alert"><Icon name="error" size={16} />{nodeChoice.unavailableReason}</p>}
            <GpuModeSupplyTargets choices={clocks.choices.filter(choice => choice.mode === mode.mode)} />
            <DetailsDialog title="风扇与系统设置的变化"><OemModeEffects target={mode} /></DetailsDialog>
          </Surface>;
        })}</div>
      </Surface>
    </section>
  </DetailsDialog>;
}
