import type { GpuModeObservation, GpuModeRawState, GpuModeDriver, GpuTurboTarget, GpuModeCycle } from "../../api/client";
import { Fragment } from "react";
import { Icon } from "../Icon";
import { FactList, type Fact } from "../FactList";
import { gpuLock, gpuMhz, gpuOffsets, gpuSkipReason } from "./gpu-values";

export function GpuModeRawStateFacts({ value }: { value: GpuModeRawState | null }) {
  if (!value) return <p className="ccc-control-caption">读取不到当前频率，也没有用保存值代替。</p>;
  return <div className="ccc-mode-clock-facts">
    <FactList compact facts={[
      { label: "图形锁频", icon: "activity", format: value.locks[0].mode === 2 ? "number" : undefined, value: gpuLock(value.locks[0]) },
      ...value.offsets.map<Fact>((offset, index) => ({ label: `GPU ${index + 1}`, icon: "gpu", value: gpuOffsets(offset) })),
      { label: "第二锁频域", icon: "activity", value: gpuLock(value.locks[1]) },
    ]} />
    {value.devices.length > 2 && <p>另有 {value.devices.length - 2} 张显卡不会被调整偏移。</p>}
  </div>;
}
export function GpuModeTargetFacts({ target }: { target: GpuTurboTarget }) {
  return <div className="ccc-mode-clock-facts">
    <FactList compact facts={[
      { label: "图形锁频", icon: "activity", format: target.lockMhz ? "number" : undefined,
        value: target.lockMhz === null ? "保持" : target.lockMhz === 0 ? "自动" : `${target.lockMhz} MHz` },
      ...(target.offsets ?? []).map<Fact>((offset, index) => ({
        label: `GPU ${index + 1}`, icon: "gpu", value: `核心 ${offset.coreMhz} MHz · 显存 ${offset.memoryMhz} MHz`,
      })),
    ]} />
    {!target.offsets && <span className="ccc-control-caption">偏移不变</span>}
  </div>;
}
export function GpuModeSupplyTargets({ choices }: { choices: GpuModeCycle["choices"] }) {
  return <table className="ccc-gpu-supply-targets" aria-label="供电频率目标">
    <thead><tr><th scope="col">供电</th><th scope="col">锁频</th><th scope="col">核心</th><th scope="col">显存</th></tr></thead>
    <tbody>{choices.map(choice => {
      const target = choice.candidate.decision?.target;
      return <Fragment key={String(choice.onAc)}><tr><th scope="row">{choice.onAc ? "插电" : "电池"}</th>
        {target ? <><td>{target.lockMhz === null ? "保持" : target.lockMhz === 0 ? "自动" : target.lockMhz}</td>
          <td>{target.offsets?.map((offset, index) => <span className="ccc-gpu-target-device" role="group" aria-label={`GPU ${index + 1} 核心频率偏移 ${offset.coreMhz} MHz`} key={index}>{offset.coreMhz}</span>) ?? "保持"}</td>
          <td>{target.offsets?.map((offset, index) => <span className="ccc-gpu-target-device" role="group" aria-label={`GPU ${index + 1} 显存频率偏移 ${offset.memoryMhz} MHz`} key={index}>{offset.memoryMhz}</span>) ?? "保持"}</td></>
          : <td colSpan={3}>{choice.candidate.unavailableReason ? "目标不可用" : "本次不调整频率"}</td>}
      </tr>{choice.candidate.unavailableReason && <tr><td colSpan={4}><span className="ccc-control-error" role="alert"><Icon name="error" size={16} />{choice.candidate.unavailableReason}</span></td></tr>}</Fragment>;
    })}</tbody>
    <caption>单位 MHz · 有多张显卡时按 GPU 顺序排列</caption>
  </table>;
}
export function GpuModeObservationFacts({ value }: { value: GpuModeObservation | null }) {
  if (value?.status === "ready") return <GpuModeRawStateFacts value={value.value} />;
  if (value?.status === "skipped") return <p className="ccc-control-caption">{gpuSkipReason(value.value)}；当前频率未读取，不沿用此前读数。</p>;
  if (value?.status === "poweredOff") return <p className="ccc-control-caption">独显关闭，当前频率未读取。</p>;
  if (value?.status === "unavailable") return <p className="ccc-control-caption">当前频率读取失败：{value.value}；未沿用此前读数。</p>;
  if (value?.status === "untracked") return <p className="ccc-control-caption">本次不会调整 GPU 频率。</p>;
  return <GpuModeRawStateFacts value={null} />;
}
export function GpuModeDriverRanges({ driver }: { driver: GpuModeDriver }) {
  return <section className="ccc-mode-clock-facts" aria-label="驱动范围">
    <strong>驱动支持的频率范围</strong>
    {driver.devices.map((device, index) => <div className="ccc-mode-clock-facts" key={`${device.id.bus}-${device.id.slot}`}>
      <strong>GPU {index + 1}</strong>
      <FactList compact facts={[
        { label: "核心", icon: "gpu", value: `${gpuMhz(device.coreRangeKhz[0])} 至 ${gpuMhz(device.coreRangeKhz[1])} · ${device.coreEditable ? "可调整" : "不可调整"}` },
        { label: "显存", icon: "memory", value: `${gpuMhz(device.memoryRangeKhz[0])} 至 ${gpuMhz(device.memoryRangeKhz[1])} · ${device.memoryEditable ? "可调整" : "不可调整"}` },
      ]} />
    </div>)}
  </section>;
}
