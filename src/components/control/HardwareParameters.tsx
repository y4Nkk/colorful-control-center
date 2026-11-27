import type { GpuClockLimitReason, HardwareSnapshot, SystemInventory } from "../../api/client";
import { freshCpuReading } from "../../app/useHardwareHistory";
import { useGpuDetails, useOverviewVisible } from "../../app/useSystemOverview";
import { BYTES_PER_MB, formatCapacity, formatParameter } from "../../lib/format-parameters";
import { FactList, type Fact } from "../FactList";
import type { IconName } from "../Icon";
import { Loading } from "../Loading";
import { ParameterDescriptions } from "./ParameterHelp";
import { DetailsDialog } from "../DetailsDialog";
import { CPU_SPEC_HELP, GPU_SPEC_HELP } from "./parameterDescriptions";

function reading(label: string, icon: IconName, value: number | null | undefined, unit: string, digits = 0): Fact {
  return { label, icon, value: formatParameter(value, unit, digits), format: Number.isFinite(value) ? "number" : undefined };
}

export function CpuParameters({ cpu, live }: { cpu: SystemInventory["cpu"] | undefined; live: HardwareSnapshot | null }) {
  const ghz = (mhz: number | null | undefined) => formatParameter(mhz == null ? null : mhz / 1000, "GHz", 2);
  return <DetailsDialog title="CPU 硬件参数" label="CPU 规格" icon={null}>
    <FactList label="CPU 参数" compact facts={[
      { label: "物理核心数", icon: "cpu", value: formatParameter(cpu?.physicalCores, "核") },
      { label: "逻辑处理器数", icon: "cpu", value: formatParameter(cpu?.logicalProcessors, "个") },
      { label: "标称基础频率", icon: "activity", value: ghz(cpu?.baseClockMhz) },
      { label: "标称最高频率", icon: "activity", value: ghz(cpu?.maxClockMhz) },
      reading("最高活跃频率（估算）", "performance", freshCpuReading(live)?.maxActiveClockMhz, "MHz"),
      reading("当前温度", "temperature", live?.fan.cpu.tempC, "°C", 1),
    ]} />
    <ParameterDescriptions items={CPU_SPEC_HELP} />
  </DetailsDialog>;
}

export function GpuParameters({ live }: { live: HardwareSnapshot | null }) {
  const gpu = live?.nvidia;
  return <DetailsDialog title="GPU 硬件参数" label="GPU 规格" icon={null}>
    <FactList label="GPU 参数" compact facts={[
      { label: "计算核心数", icon: "gpu", value: formatParameter(gpu?.cudaCores, "个") },
      { label: "显存位宽", icon: "memory", value: formatParameter(gpu?.memoryBusWidthBits, "位") },
      { label: "显存总量", icon: "memory", value: formatCapacity(gpu?.memTotalMb == null ? null : gpu.memTotalMb * BYTES_PER_MB) },
      reading("显存容量占用率", "memory", gpu?.memoryUsedPct, "%", 1),
      reading("当前核心频率", "activity", gpu?.graphicsClockMhz, "MHz"),
      reading("当前显存频率", "memory", gpu?.memoryClockMhz, "MHz"),
      reading("当前温度", "temperature", gpu?.tempC, "°C", 1),
      reading("当前功耗", "power", gpu?.powerDrawW, "W", 1),
      { label: "当前功耗上限", icon: "power", value: formatParameter(gpu?.powerLimitW, "W", 1) },
    ]} />
    {gpu && !live?.nvidiaSuspended && <VisibleGpuReadings key={gpu.connectionId} connectionId={gpu.connectionId} />}
    <ParameterDescriptions items={GPU_SPEC_HELP} />
  </DetailsDialog>;
}

const clockLimitLabels: Record<GpuClockLimitReason, string> = {
  idle: "GPU 空闲",
  applicationClocks: "应用频率设置",
  powerCap: "功耗限制",
  hardwareSlowdown: "硬件保护",
  syncBoost: "同步加速",
  softwareTemperature: "软件温度限制",
  hardwareTemperature: "硬件温度限制",
  powerBrake: "供电限制",
  displayClocks: "显示频率设置",
  unknown: "未识别原因",
};

function formatLink(generation: number | null, width: number | null) {
  return [generation == null ? null : `PCIe ${generation}.0`, width == null ? null : `×${width}`]
    .filter(Boolean).join(" ") || "未知";
}

function VisibleGpuReadings({ connectionId }: { connectionId: number }) {
  return useOverviewVisible() ? <GpuReadings connectionId={connectionId} /> : null;
}

function GpuReadings({ connectionId }: { connectionId: number }) {
  const details = useGpuDetails(connectionId);
  if (details === undefined) return <Loading label="正在读取 GPU 详情…" />;
  if (details === null) return <p role="status">GPU 运行详情暂不可用</p>;
  const reasons = details.clockLimitReasons;
  const facts: Fact[] = [
    reading("视频编码占用率", "activity", details.encoderUtilizationPct, "%"),
    reading("视频解码占用率", "activity", details.decoderUtilizationPct, "%"),
    { label: "性能状态", icon: "performance", format: details.performanceState == null ? undefined : "code",
      value: details.performanceState == null ? "未知" : `P${details.performanceState}` },
    { label: "当前链路", icon: "gpu", value: formatLink(details.pcieLinkGeneration, details.pcieLinkWidth) },
    { label: "最大链路", icon: "gpu", value: formatLink(details.pcieMaxLinkGeneration, details.pcieMaxLinkWidth) },
    { label: "限频原因", icon: "info", value: reasons == null ? "未知" : reasons.map(reason => clockLimitLabels[reason]).join("、") || "无" },
  ];
  if (details.memoryTemperatureC != null) facts.push(reading("显存温度", "temperature", details.memoryTemperatureC, "°C", 1));
  if (details.fanTargetRpm != null) facts.push(reading("GPU 目标风扇转速", "fan", details.fanTargetRpm, "RPM"));
  return <FactList compact facts={facts} label="GPU 运行详情" />;
}
