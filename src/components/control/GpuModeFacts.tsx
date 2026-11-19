import type { GpuModeObservation, GpuModeRawState, GpuModeDriver, GpuTurboTarget, GpuModeCycle } from "../../api/client";
import { DetailsDialog } from "../DetailsDialog";
import { Fragment } from "react";
import { gpuLock, gpuMhz, gpuOffsets, gpuSkipReason } from "./gpu-values";

export function GpuModeRawStateFacts({ value }: { value: GpuModeRawState | null }) {
  if (!value) return <p className="ccc-control-caption">当前频率未知，未补用保存值。</p>;
  return <div className="ccc-mode-clock-facts ccc-gpu-clock-readings">
    <p className="ccc-gpu-clock-main"><span className="ccc-control-caption">图形锁频：</span><strong>{gpuLock(value.locks[0])}</strong></p>
    {value.offsets.map((offset, index) => <p key={index} className="ccc-gpu-clock-offset"><span className="ccc-control-caption">GPU {index + 1}</span><span className="ccc-mono">{gpuOffsets(offset)}</span></p>)}
    <DetailsDialog title="更多频率细节"><p>第二锁频域：{gpuLock(value.locks[1])}</p>
      {value.devices.length > 2 && <p>另有 {value.devices.length - 2} 张显卡不在偏移目标内。</p>}
    </DetailsDialog>
  </div>;
}
export function GpuModeTargetFacts({ target }: { target: GpuTurboTarget }) {
  return <div className="ccc-mode-clock-facts ccc-gpu-clock-readings">
    <p className="ccc-gpu-clock-main"><span className="ccc-control-caption">图形锁频：</span><strong>{target.lockMhz === null ? "保持" : target.lockMhz === 0 ? "自动" : `${target.lockMhz} MHz`}</strong></p>
    {target.offsets ? target.offsets.map((offset, index) => <p key={index} className="ccc-gpu-clock-offset">
      <span className="ccc-control-caption">GPU {index + 1}</span><span className="ccc-mono">核心 {offset.coreMhz} MHz · 显存 {offset.memoryMhz} MHz</span>
    </p>) : <span className="ccc-control-caption">偏移保持</span>}
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
          : <td colSpan={3}>{choice.candidate.unavailableReason ? "目标不可用" : "不调频"}</td>}
      </tr>{choice.candidate.unavailableReason && <tr><td colSpan={4}><span className="ccc-control-error" role="alert">{choice.candidate.unavailableReason}</span></td></tr>}</Fragment>;
    })}</tbody>
    <caption>MHz · 多卡按 GPU 顺序</caption>
  </table>;
}
export function GpuModeObservationFacts({ value }: { value: GpuModeObservation | null }) {
  if (value?.status === "ready") return <GpuModeRawStateFacts value={value.value} />;
  if (value?.status === "skipped") return <p className="ccc-control-caption">{gpuSkipReason(value.value)}；当前频率未读取，不沿用此前读数。</p>;
  if (value?.status === "poweredOff") return <p className="ccc-control-caption">独显关闭，当前频率未读取。</p>;
  if (value?.status === "unavailable") return <p className="ccc-control-caption">当前频率读取失败：{value.value}；未沿用此前读数。</p>;
  if (value?.status === "untracked") return <p className="ccc-control-caption">本策略未接管GPU频率。</p>;
  return <GpuModeRawStateFacts value={null} />;
}
export function GpuModeDriverRanges({ driver }: { driver: GpuModeDriver }) {
  return <DetailsDialog title="查看驱动范围">{driver.devices.map((device, index) => <div key={`${device.id.bus}-${device.id.slot}`}>
    <strong>GPU {index + 1}</strong>
    <p>核心 {gpuMhz(device.coreRangeKhz[0])} 至 {gpuMhz(device.coreRangeKhz[1])} · {device.coreEditable ? "可编辑" : "不可编辑"}</p>
    <p>显存 {gpuMhz(device.memoryRangeKhz[0])} 至 {gpuMhz(device.memoryRangeKhz[1])} · {device.memoryEditable ? "可编辑" : "不可编辑"}</p>
  </div>)}</DetailsDialog>;
}
