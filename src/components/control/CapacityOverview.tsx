import { Surface } from "../Surface";
import { Icon } from "../Icon";
import type { HardwareCapacity, OemMemoryReading, SystemOverview } from "../../api/client";
import type { MemorySample } from "../../app/useSystemOverview";
import { HardwareArtwork } from "./HardwareArtwork";
import { FactList } from "../FactList";
import { formatCapacity, formatParameter } from "../../lib/format-parameters";
import { DetailsDialog } from "../DetailsDialog";
import { ParameterDescriptions } from "./ParameterHelp";
import { MEMORY_HELP, STORAGE_HELP } from "./parameterDescriptions";
import { MemoryTrend } from "./MemoryTrend";
import { fmtInt } from "./format";

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
  return <svg className="ccc-capacity-bar" viewBox="0 0 100 10" preserveAspectRatio="none" role="meter"
    aria-label={label} aria-valuemin={0} aria-valuemax={values.total} aria-valuenow={values.used}
    aria-valuetext={`已用 ${formatCapacity(values.used)}，可用 ${formatCapacity(values.available)}，共 ${formatCapacity(values.total)}`}>
    <rect className="ccc-capacity-track" width="100" height="10" />
    <rect className="ccc-capacity-fill" width={values.usedPercent} height="10" />
  </svg>;
}

function MemoryUsage({ values, history }: { values: CapacityValues | null; history: readonly MemorySample[] }) {
  const points = history.map(({ atMs, capacity }) => ({ atMs, value: capacityValues(capacity)?.usedPercent ?? null }));
  return <div className="ccc-memory-usage">
    {values ? <dl className="ccc-memory-stats">
      <div className="ccc-memory-primary">
        <dt>已用内存</dt>
        <dd>
          <span aria-label="内存已用容量">{formatCapacity(values.used)}</span>
          <span className="ccc-memory-percent" aria-label="当前内存使用率">{fmtInt(values.usedPercent)}%</span>
        </dd>
      </div>
      <div><dt>可用</dt><dd>{formatCapacity(values.available)}</dd></div>
      <div><dt>总容量</dt><dd>{formatCapacity(values.total)}</dd></div>
    </dl> : <p className="ccc-capacity-empty">容量未知</p>}
    <MemoryTrend points={points} />
  </div>;
}

function StorageVolume({ volume }: { volume: SystemOverview["volumes"][number] }) {
  const values = capacityValues(volume.capacity);
  const name = volume.label || "本地磁盘";
  return <li className="ccc-storage-volume">
    <span className="ccc-storage-icon"><Icon name="storage" size={24} /></span>
    <div className="ccc-storage-info">
      <h4 title={`${name} (${volume.mount})`}><span>{name}</span> <span>({volume.mount})</span></h4>
      {values ? <>
        <CapacityBar values={values} label={`${volume.mount} 使用量`} />
        <span className="ccc-capacity-available">{formatCapacity(values.available)} 可用 · 共 {formatCapacity(values.total)}</span>
      </> : <span className="ccc-capacity-available">容量未知</span>}
    </div>
  </li>;
}

export function CapacityOverview({ overview, oemMemory, memoryHistory }: {
  overview: SystemOverview | null; oemMemory: OemMemoryReading | null; memoryHistory: readonly MemorySample[];
}) {
  const memory = capacityValues(overview?.memory);
  const volumes = overview?.volumes ?? [];
  return <div className="ccc-module-grid">
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="系统内存概览">
      <div className="ccc-capacity-head">
        <div className="ccc-capacity-heading"><h3>系统内存</h3>
          <DetailsDialog title="内存详细信息">
            <FactList compact facts={[
              { label: "总容量", value: formatCapacity(memory?.total) },
              { label: "已用容量", value: formatCapacity(memory?.used) },
              { label: "可用容量", value: formatCapacity(memory?.available) },
              { label: "原厂报告频率", value: formatParameter(oemMemory?.reportedClockMhz, "MHz") },
              { label: "原厂参考频率", value: formatParameter(oemMemory?.referenceClockMhz, "MHz") },
            ]} />
            <ParameterDescriptions items={MEMORY_HELP} />
          </DetailsDialog>
        </div>
        <HardwareArtwork kind="memory" />
      </div>
      <MemoryUsage values={memory} history={memoryHistory} />
    </Surface>
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="本地存储概览">
      <div className="ccc-capacity-head">
        <div className="ccc-capacity-heading"><h3>本地存储</h3>
          <DetailsDialog title="存储详细信息">
            {volumes.map(volume => {
              const values = capacityValues(volume.capacity);
              return <section key={volume.mount} className="ccc-review-section">
                <h3>{volume.mount}</h3>
                <FactList compact facts={[
                  { label: "卷标", value: volume.label == null ? "未知" : volume.label || "未设置" },
                  { label: "文件系统", value: volume.fileSystem || "未知" },
                  { label: "总容量", value: formatCapacity(values?.total) },
                  { label: "已用容量", value: formatCapacity(values?.used) },
                  { label: "可用容量", value: formatCapacity(values?.available) },
                ]} />
              </section>;
            })}
            <ParameterDescriptions items={STORAGE_HELP} />
          </DetailsDialog>
        </div>
        <HardwareArtwork kind="storage" />
      </div>
      {volumes.length ? <ul className="ccc-storage-volumes">
        {volumes.map(volume => <StorageVolume volume={volume} key={volume.mount} />)}
      </ul> : <p className="ccc-capacity-empty">{overview ? "暂无固定磁盘卷" : "容量未读取"}</p>}
    </Surface>
  </div>;
}
