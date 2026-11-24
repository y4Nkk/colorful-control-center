import type { HardwareSnapshot, SystemOverview } from "../../api/client";
import { BYTES_PER_MB, formatCapacity, formatParameter } from "../../lib/format-parameters";
import { FactList } from "../FactList";
import { ParameterDescriptions } from "./ParameterHelp";
import { DetailsDialog } from "../DetailsDialog";
import { CPU_SPEC_HELP, GPU_SPEC_HELP } from "./parameterDescriptions";

export function CpuParameters({ overview, live }: { overview: SystemOverview | null; live: HardwareSnapshot | null }) {
  const cpu = overview?.cpu;
  const ghz = (mhz: number | null | undefined) => formatParameter(mhz == null ? null : mhz / 1000, "GHz", 2);
  return <DetailsDialog title="CPU 硬件参数" label="CPU 规格" icon={null}>
    <FactList compact facts={[
      { label: "物理核心数", value: formatParameter(cpu?.physicalCores, "核") },
      { label: "逻辑处理器数", value: formatParameter(cpu?.logicalProcessors, "个") },
      { label: "标称基础频率", value: ghz(cpu?.baseClockMhz) },
      { label: "标称最高频率", value: ghz(cpu?.maxClockMhz) },
      { label: "当前温度", value: formatParameter(live?.fan.cpu.tempC, "°C", 1) },
    ]} />
    <ParameterDescriptions items={CPU_SPEC_HELP} />
  </DetailsDialog>;
}

export function GpuParameters({ live }: { live: HardwareSnapshot | null }) {
  const gpu = live?.nvidia;
  return <DetailsDialog title="GPU 硬件参数" label="GPU 规格" icon={null}>
    <FactList compact facts={[
      { label: "计算核心数", value: formatParameter(gpu?.cudaCores, "个") },
      { label: "显存位宽", value: formatParameter(gpu?.memoryBusWidthBits, "位") },
      { label: "显存总量", value: formatCapacity(gpu?.memTotalMb == null ? null : gpu.memTotalMb * BYTES_PER_MB) },
      { label: "当前核心频率", value: formatParameter(gpu?.graphicsClockMhz, "MHz") },
      { label: "当前显存频率", value: formatParameter(gpu?.memoryClockMhz, "MHz") },
      { label: "当前温度", value: formatParameter(gpu?.tempC, "°C", 1) },
      { label: "当前功耗", value: formatParameter(gpu?.powerDrawW, "W", 1) },
      { label: "当前功耗上限", value: formatParameter(gpu?.powerLimitW, "W", 1) },
    ]} />
    <ParameterDescriptions items={GPU_SPEC_HELP} />
  </DetailsDialog>;
}
