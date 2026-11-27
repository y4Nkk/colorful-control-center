import { memo } from "react";
import { Surface } from "../Surface";
import { Icon } from "../Icon";
import type { HardwareCapacity, MemoryModule, SystemInventory } from "../../api/client";
import type { MemorySample } from "../../app/useSystemOverview";
import { HardwareCardHeader } from "./HardwareCardHeader";
import { FactList } from "../FactList";
import { formatCapacity, formatParameter } from "../../lib/format-parameters";
import { DetailsDialog } from "../DetailsDialog";
import { Tooltip } from "../Tooltip";
import { ParameterDescriptions } from "./ParameterHelp";
import { MEMORY_HELP, STORAGE_HELP } from "./parameterDescriptions";
import { MemoryTrend } from "./MemoryTrend";
import { MetricReadout } from "./MetricReadout";

interface CapacityValues {
  total: number;
  available: number;
  used: number;
  usedPercent: number;
}

function capacityValues(capacity: HardwareCapacity | null | undefined): CapacityValues | null {
  if (!capacity || !Number.isFinite(capacity.totalBytes) || !Number.isFinite(capacity.availableBytes)
    || capacity.totalBytes <= 0 || capacity.availableBytes < 0 || capacity.availableBytes > capacity.totalBytes) return null;
  const total = capacity.totalBytes, available = capacity.availableBytes;
  const used = total - available;
  return { total, available, used, usedPercent: used / total * 100 };
}

function CapacityBar({ values, label }: { values: CapacityValues; label: string }) {
  return <div className="ccc-capacity-bar" role="meter"
    aria-label={label} aria-valuemin={0} aria-valuemax={values.total} aria-valuenow={values.used}
    aria-valuetext={`已用 ${formatCapacity(values.used)}，可用 ${formatCapacity(values.available)}，共 ${formatCapacity(values.total)}`}>
    <span className="ccc-capacity-fill" style={{ inlineSize: `${values.usedPercent}%` }} />
  </div>;
}

function MemoryUsage({ values, speedMts, history }: { values: CapacityValues | null; speedMts: number | null; history: readonly MemorySample[] }) {
  const points = history.map(({ atMs, capacity }) => ({ atMs, value: capacityValues(capacity)?.usedPercent ?? null }));
  const color = "var(--ccc-chart-purple)";
  return <>
    <div className="ccc-overview-metrics">
      <div className="ccc-overview-metric-grid">
        <MetricReadout compact label="已用内存" ariaLabel="内存已用容量" value={values?.used ?? null} unit="bytes" emptyText="未知" />
        <MetricReadout compact label="可用" value={values?.available ?? null} unit="bytes" emptyText="未知" />
        <MetricReadout compact label="总容量" value={values?.total ?? null} unit="bytes" emptyText="未知" />
        <MetricReadout compact label="频率" ariaLabel="内存频率" value={speedMts} unit="MT/s" emptyText="未知" />
      </div>
    </div>
    <div className="ccc-overview-charts">
      <div className="ccc-overview-utilization">
        <MetricReadout ratioShape="arc" label="使用率" ariaLabel="当前内存使用率"
          value={values?.usedPercent ?? null} unit="%" maximum={100} colorVar={color} />
      </div>
      <MemoryTrend points={points} color={color} />
    </div>
  </>;
}

function usageLevel(shownPercent: number): "warning" | "alert" | undefined {
  if (shownPercent >= 90) return "alert";
  if (shownPercent >= 80) return "warning";
  return undefined;
}

function StorageVolume({ volume }: { volume: NonNullable<SystemInventory["volumes"]>[number] }) {
  const values = capacityValues(volume.capacity);
  const shownPercent = values ? Math.round(values.usedPercent) : null;
  const name = volume.label || "本地磁盘";
  return <li className="ccc-storage-volume" data-usage={shownPercent != null && usageLevel(shownPercent) || undefined}>
    <span className="ccc-storage-icon"><Icon name="storage" size={24} /></span>
    <div className="ccc-storage-info">
      <div className="ccc-storage-heading">
        <Tooltip content={`${name} (${volume.mount})`}>
          <h4><span>{name}</span> <span>({volume.mount})</span></h4>
        </Tooltip>
        {values && <span className="ccc-capacity-available ccc-mono" aria-hidden="true">{shownPercent}%</span>}
      </div>
      {values ? <>
        <CapacityBar values={values} label={`${volume.mount} 使用量`} />
        <span className="ccc-capacity-available">{formatCapacity(values.available)} 可用 · 共 {formatCapacity(values.total)}</span>
      </> : <span className="ccc-capacity-available">容量未知</span>}
    </div>
  </li>;
}

function memoryModuleValue(module: MemoryModule) {
  return [
    module.soldered ? "板载" : null,
    module.memoryType,
    module.capacityBytes == null ? null : formatCapacity(module.capacityBytes),
    module.configuredSpeedMts == null ? null : formatParameter(module.configuredSpeedMts, "MT/s"),
    module.manufacturer,
    module.partNumber,
  ].filter(Boolean).join(" · ") || "未知";
}

export const CapacityOverview = memo(function CapacityOverview({ inventory, capacity, memoryHistory, reportedClockMhz, referenceClockMhz }: {
  inventory: SystemInventory | null; capacity: HardwareCapacity | null; memoryHistory: readonly MemorySample[];
  reportedClockMhz: number | null; referenceClockMhz: number | null;
}) {
  const memory = capacityValues(capacity);
  const storage = inventory?.volumes;
  const volumes = storage ?? [];
  const speedMts = inventory?.memorySpeedMts ?? null;
  const slots = inventory?.memorySlots;
  return <div className="ccc-module-grid">
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="系统内存概览">
      <HardwareCardHeader kind="memory" title="系统内存">
        <DetailsDialog title="内存详细信息" label="内存详情">
          <FactList compact facts={[
            { label: "总容量", icon: "memory", value: formatCapacity(memory?.total) },
            { label: "已用容量", icon: "memory", format: memory ? "number" : undefined, value: formatCapacity(memory?.used) },
            { label: "可用容量", icon: "memory", value: formatCapacity(memory?.available) },
            { label: "内存频率", icon: "activity", format: speedMts == null ? undefined : "number", value: formatParameter(speedMts, "MT/s") },
            { label: "原厂报告频率", value: formatParameter(reportedClockMhz, "MHz") },
            { label: "原厂参考频率", value: formatParameter(referenceClockMhz, "MHz") },
            ...(slots ? [{ label: "已用内存插槽", value: `${slots.used}/${slots.total}` }] : []),
            ...(inventory?.memoryModules ?? []).map((module, index) => ({
              label: module.locator ?? `内存设备 ${index + 1}`,
              value: memoryModuleValue(module),
            })),
          ]} />
          <ParameterDescriptions items={MEMORY_HELP} />
        </DetailsDialog>
      </HardwareCardHeader>
      <MemoryUsage values={memory} speedMts={speedMts} history={memoryHistory} />
    </Surface>
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="本地存储概览">
      <HardwareCardHeader kind="storage" title="本地存储">
        <DetailsDialog title="存储详细信息" label="存储详情">
          {volumes.map(volume => {
            const values = capacityValues(volume.capacity);
            return <section key={volume.mount} className="ccc-review-section">
              <h3>{volume.mount}</h3>
              <FactList compact facts={[
                { label: "卷标", icon: "storage", value: volume.label == null ? "未知" : volume.label || "未设置" },
                { label: "文件系统", icon: "logs", format: volume.fileSystem ? "code" : undefined, value: volume.fileSystem || "未知" },
                { label: "总容量", value: formatCapacity(values?.total) },
                { label: "已用容量", format: values ? "number" : undefined, value: formatCapacity(values?.used) },
                { label: "可用容量", value: formatCapacity(values?.available) },
              ]} />
            </section>;
          })}
          <ParameterDescriptions items={STORAGE_HELP} />
        </DetailsDialog>
      </HardwareCardHeader>
      {volumes.length ? <ul className="ccc-storage-volumes">
        {volumes.map(volume => <StorageVolume volume={volume} key={volume.mount} />)}
      </ul> : <p className="ccc-capacity-empty">{storage ? "暂无固定磁盘卷" : "容量未读取"}</p>}
    </Surface>
  </div>;
});
