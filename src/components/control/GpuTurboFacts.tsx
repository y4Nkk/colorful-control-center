import { OEM_POWER_MODES, type GpuTurboProfile } from "../../api/client";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { FactList, type Fact } from "../FactList";
import { gpuLock, gpuOffsets, gpuRestoredOffset, gpuTarget } from "./gpu-values";

export function GpuTurboFacts({ profile }: { profile: GpuTurboProfile }) {
  const { current, initial } = profile;
  return <section className="ccc-gpu-turbo-facts" aria-label="原厂 GPU Turbo 事实与预览">
    <strong>原厂 GPU Turbo</strong>
    <div className="ccc-gpu-turbo-columns">
      <Surface variant="inset" className="ccc-gpu-turbo-part">
        <strong>当前显卡</strong>
        {current ? <>
          <p className="ccc-control-caption">{OEM_POWER_MODES[current.source.mode]} · {current.source.onAc === null ? "供电未知" : current.source.onAc ? "插电" : "电池"}</p>
          {current.source.supplyError && <p className="ccc-control-error">供电状态：{current.source.supplyError}</p>}
          <FactList label="当前显卡参数" compact facts={[
            { label: "图形锁频", icon: "activity", format: current.locks[0].mode === 2 ? "number" : undefined, value: gpuLock(current.locks[0]) },
            { label: "第二锁频域", icon: "activity", value: gpuLock(current.locks[1]) },
            ...current.devices.map<Fact>((gpu, index) => ({ label: `GPU ${index + 1}`, icon: "gpu", value: gpuOffsets(gpu.offsets) })),
          ]} />
          {current.otherDevices.length > 0 && <p className="ccc-control-caption">另有 {current.otherDevices.length} 张显卡不在原厂偏移目标内。</p>}
        </> : <p className="ccc-control-caption">读取不到当前显卡的状态。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
        {initial && <>
          <strong>恢复接管前的值</strong>
          <GpuTurboRestorePreview profile={profile} />
        </>}
      </Surface>
      <Surface variant="inset" className="ccc-gpu-turbo-part">
        <strong>接管后会设置的目标</strong>
        {current ? <>
          {!current.currentOn && <p className="ccc-control-caption">当前模式的供电状态未知，不会执行任何目标。</p>}
          <FactList label="各模式的 GPU Turbo 目标" compact facts={[
            { label: "关闭", value: <>{gpuTarget(current.off)}
              {current.offError && <div className="ccc-control-error">关闭事件不可执行：{current.offError}</div>}</> },
            ...current.onTargets.filter(item => item.onAc || item.mode === 2).map(item => ({
              label: `${OEM_POWER_MODES[item.mode] ?? ""}${item.mode === 2 ? item.onAc ? " · 插电" : " · 电池" : ""}`,
              value: <>{gpuTarget(item.target)}
                {item.unavailableReason && <div className="ccc-control-error">不可执行：{item.unavailableReason}</div>}</>,
            })),
          ]} />
          <DetailsDialog title="锁频与恢复说明">
            <p>打开 GPU Turbo 时，会按当前预设模式和供电状态选用对应设置。</p>
            <p>「锁频」是把显卡核心频率固定在指定数值。执行锁频时，会同时解除第二个频率域的锁定。恢复时，两项锁频设置都会回到接管前的值。</p>
            <p>显卡、驱动支持的范围或保存的目标发生变化后，需要重新确认接管。</p>
          </DetailsDialog>
        </> : <p className="ccc-control-caption">目标不可用，不能确认接管这项动作。</p>}
      </Surface>
    </div>
  </section>;
}

export function GpuTurboRestorePreview({ profile }: { profile: GpuTurboProfile }) {
  const { initial, restoreCurrent, restoreError } = profile;
  if (!initial) return null;
  const before = restoreCurrent?.locks;
  return <>
    <FactList label="恢复锁频与偏移" compact facts={[
      { label: "图形锁频", value: initial.locks ? `${before ? gpuLock(before[0]) : "未知"} → ${gpuLock(initial.locks[0])}` : "未接管" },
      ...(initial.locks ? [{ label: "第二锁频域", value: `${before ? gpuLock(before[1]) : "未知"} → ${gpuLock(initial.locks[1])}` }] : []),
      ...initial.offsets.map((value, index) => ({
        label: `GPU ${index + 1}`,
        value: `核心 ${gpuRestoredOffset(restoreCurrent?.offsets[index]?.coreKhz, value.coreKhz)} · 显存 ${gpuRestoredOffset(restoreCurrent?.offsets[index]?.memKhz, value.memKhz)}`,
      })),
    ]} />
    {restoreError && <p className="ccc-control-error">{restoreError}</p>}
  </>;
}
