import { restoreGpuTurbo, type GpuTurboProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { gpuLock, gpuOffsets, gpuRestoredOffset, gpuTarget } from "./gpu-values";

type Props = { profile: GpuTurboProfile; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
const modes = ["安静", "省电", "性能", "娱乐", "静音增强"];

export function GpuTurboFacts({ profile, busy, run }: Props) {
  const { current, initial } = profile;
  return <section className="ccc-gpu-turbo-facts" aria-label="原厂 GPU Turbo 事实与预览">
    <strong>原厂 GPU Turbo</strong>
    <div className="ccc-gpu-turbo-columns">
      <Surface variant="inset" className="ccc-gpu-turbo-part">
        <strong>当前显卡</strong>
        {current ? <>
          <p className="ccc-control-caption">{modes[current.source.mode]} · {current.source.onAc === null ? "供电未知" : current.source.onAc ? "插电" : "电池"}</p>
          {current.source.supplyError && <p className="ccc-control-error">供电状态：{current.source.supplyError}</p>}
          <p>图形锁频：{gpuLock(current.locks[0])}</p>
          <p className="ccc-control-caption">第二锁频域：{gpuLock(current.locks[1])}</p>
          {current.devices.map((gpu, index) => <p key={`${gpu.id.bus}-${gpu.id.slot}`}>GPU {index + 1} · {gpuOffsets(gpu.offsets)}</p>)}
          {current.otherDevices.length > 0 && <p className="ccc-control-caption">另有 {current.otherDevices.length} 张显卡不在原厂偏移目标内。</p>}
        </> : <p className="ccc-control-caption">当前显卡事实不可用。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
        {initial && <>
          <strong>初始恢复目标</strong>
          <p className="ccc-control-caption">图形锁频：{initial.locks ? `${profile.restoreCurrent?.locks ? gpuLock(profile.restoreCurrent.locks[0]) : "未知"} → ${gpuLock(initial.locks[0])}` : "未接管"}</p>
          {initial.locks && <p className="ccc-control-caption">第二锁频域：{profile.restoreCurrent?.locks ? gpuLock(profile.restoreCurrent.locks[1]) : "未知"} → {gpuLock(initial.locks[1])}</p>}
          {initial.offsets.map((value, index) => <p className="ccc-control-caption" key={index}>
            GPU {index + 1} · 核心 {gpuRestoredOffset(profile.restoreCurrent?.offsets[index]?.coreKhz, value.coreKhz)} · 显存 {gpuRestoredOffset(profile.restoreCurrent?.offsets[index]?.memKhz, value.memKhz)}
          </p>)}
          {profile.restoreError && <p className="ccc-control-error">{profile.restoreError}</p>}
          <ConfirmButton icon="undo" key={profile.restoreRevision ?? "gpu-turbo-unavailable"} variant="secondary"
            label="恢复初始 GPU 状态" confirmLabel="确认停止监听并恢复上述状态？" busy={busy} disabled={!profile.restoreRevision}
            onConfirm={() => run(() => restoreGpuTurbo(profile.restoreRevision!), "初始 GPU 状态已恢复", false)} />
        </>}
      </Surface>
      <Surface variant="inset" className="ccc-gpu-turbo-part">
        <strong>接管后的事件目标</strong>
        {current ? <>
          <p className="ccc-control-caption">关闭：{gpuTarget(current.off)}</p>
          {current.offError && <p className="ccc-control-error">关闭事件不可执行：{current.offError}</p>}
          {!current.currentOn && <p className="ccc-control-caption">当前模式的供电状态未知；不会猜测执行目标。</p>}
          <ul className="ccc-gpu-turbo-modes">{current.onTargets.filter(item => item.onAc || item.mode === 2).map(item =>
            <li key={`${item.mode}-${item.onAc}`}><span>{modes[item.mode]}{item.mode === 2 ? item.onAc ? " · 插电" : " · 电池" : ""}</span>
              <span className="ccc-gpu-turbo-target">{gpuTarget(item.target)}</span>{item.unavailableReason && <span className="ccc-control-error">不可执行：{item.unavailableReason}</span>}</li>)}</ul>
          <DetailsDialog title="锁频与恢复说明"><p>锁频操作同时解除第二锁频域，恢复时还原初始值。模式或供电变化时按上表选择；设备、能力或保存目标变化后需要重新确认。</p></DetailsDialog>
        </> : <p className="ccc-control-caption">目标不可用，不能确认此范围。</p>}
      </Surface>
    </div>
  </section>;
}
