import type { OemModeRestoreTarget } from "../../api/client";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { GpuModeRawStateFacts, GpuModeObservationFacts } from "./GpuModeFacts";

export function OemModeGpuRestore({ target }: { target: OemModeRestoreTarget }) {
  const initial = target.gpu;
  if (!initial) return null;
  const current = target.currentGpuPower;
  const origin = initial.clocks?.origin;
  return <section className="ccc-mode-restore-block" aria-label="GPU初始恢复目标">
    <h3>GPU恢复</h3>
    {initial.node && <FactList compact layout="tiles" facts={[
      { label: "独显节点", value: `${current ? current.enabled ? "开启" : "关闭" : "未知"} → ${initial.node.target.enabled ? "开启" : "关闭"}` },
      { label: "自动关断", value: `${current ? current.automaticPowerOff ? "允许" : "关闭" : "未知"} → ${initial.node.target.automaticPowerOff ? "允许" : "关闭"}` },
    ]} />}
    {initial.clocks && <div className="ccc-mode-gpu-columns">
      <Surface variant="inset" className="ccc-mode-gpu-part"><strong>当前频率</strong><GpuModeObservationFacts value={target.currentGpuClocks} /></Surface>
      <Surface variant="inset" className="ccc-mode-gpu-part"><strong>初始频率目标</strong><GpuModeRawStateFacts value={initial.clocks.state} /></Surface>
    </div>}
    {origin && <p className="ccc-control-caption">恢复频率来源：{origin === "afterEnable" ? "首次开启独显后的实际读数" : origin === "poweredOff" ? "独显关断期间的实际读数" : "模式切换前的实际读数"}。</p>}
    {target.currentGpuClocks.status === "poweredOff" && initial.clocks && <p className="ccc-control-caption">当前频率未读取；确认后按恢复顺序核对驱动，通过后才恢复频率。</p>}
  </section>;
}
