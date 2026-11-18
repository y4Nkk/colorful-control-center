import { Surface } from "../Surface";
import type { HardwareCapacity, OemMemoryReading, SystemOverview } from "../../api/client";
import { HardwareArtwork } from "./HardwareArtwork";
import { FactList } from "../FactList";
import { formatCapacity, formatParameter } from "../../lib/format-parameters";
import { DetailsDialog } from "../DetailsDialog";
import { ParameterDescriptions } from "./ParameterHelp";
import { MEMORY_HELP, STORAGE_HELP } from "./parameterDescriptions";

const gib = (bytes: number) => bytes / 1024 ** 3;
const capacityText = (bytes: number) => gib(bytes).toFixed(1);

function capacityValues(capacity: HardwareCapacity | null | undefined) {
  if (!capacity || !Number.isFinite(capacity.totalBytes) || !Number.isFinite(capacity.availableBytes)
    || capacity.totalBytes <= 0 || capacity.availableBytes < 0 || capacity.availableBytes > capacity.totalBytes) return null;
  return { total: capacity.totalBytes, available: capacity.availableBytes, used: capacity.totalBytes - capacity.availableBytes };
}

function CapacityUsage({ capacity, label }: { capacity: HardwareCapacity | null; label: string }) {
  const values = capacityValues(capacity);
  const total = values?.total;
  const valid = values != null;
  const used = values?.used ?? null;
  const ratio = values ? values.used / values.total * 100 : null;
  return <div className="ccc-capacity-usage">
    <div className="ccc-capacity-ring ccc-meter-ring" role={valid ? "meter" : "img"} aria-label={`${label}使用量`}
      aria-valuemin={valid ? 0 : undefined} aria-valuemax={valid ? total : undefined} aria-valuenow={used ?? undefined}
      aria-valuetext={values ? `已用 ${capacityText(values.used)} GiB，共 ${capacityText(values.total)} GiB` : undefined}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle className="ccc-meter-ring-track" cx="40" cy="40" r="35" />
        {ratio != null && ratio > 0 && <circle className="ccc-meter-ring-fill" cx="40" cy="40" r="35"
          pathLength="100" strokeDasharray={`${ratio} 100`} transform="rotate(-90 40 40)" />}
      </svg>
      <span aria-hidden="true">{ratio == null ? "—" : `${Math.round(ratio)}%`}</span>
    </div>
    <div>
      <div className="ccc-capacity-value ccc-mono">
        <output aria-label={`${label}已用容量`}>{used == null ? "—" : capacityText(used)}</output>
        <span>{values ? `/ ${capacityText(values.total)} GiB` : "容量未知"}</span>
      </div>
      <span className="ccc-capacity-available">{values ? `可用 ${capacityText(values.available)} GiB` : "暂未读取到容量"}</span>
    </div>
  </div>;
}

export function CapacityOverview({ overview, oemMemory }: {
  overview: SystemOverview | null; oemMemory: OemMemoryReading | null;
}) {
  const memory = capacityValues(overview?.memory);
  return <div className="ccc-capacity-overview">
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="系统内存概览">
      <div className="ccc-capacity-head">
        <div><h3>系统内存</h3>
        <DetailsDialog title="内存详细信息">
          <FactList compact facts={[
            { label: "总容量", value: formatCapacity(memory?.total) },
            { label: "已用容量", value: formatCapacity(memory?.used) },
            { label: "可用容量", value: formatCapacity(memory?.available) },
            { label: "原厂报告频率", value: formatParameter(oemMemory?.reportedClockMhz, "MHz") },
            { label: "原厂参考频率", value: formatParameter(oemMemory?.referenceClockMhz, "MHz") },
          ]} />
          <ParameterDescriptions items={MEMORY_HELP} />
        </DetailsDialog></div>
        <HardwareArtwork kind="memory" />
      </div>
      <CapacityUsage capacity={overview?.memory ?? null} label="内存" />
    </Surface>
    <Surface as="section" variant="card" className="ccc-capacity-card" aria-label="本地存储概览">
      <div className="ccc-capacity-head">
        <div><h3>本地存储</h3>
        <DetailsDialog title="存储详细信息">
          {overview?.volumes.map(volume => <section key={volume.mount} className="ccc-review-section">
            <h3>{volume.mount}</h3>
            <FactList compact facts={[
              { label: "卷标", value: volume.label == null ? "未知" : volume.label || "未设置" },
              { label: "文件系统", value: volume.fileSystem || "未知" },
              { label: "总容量", value: formatCapacity(capacityValues(volume.capacity)?.total) },
            ]} />
          </section>)}
          <ParameterDescriptions items={STORAGE_HELP} />
        </DetailsDialog></div>
        <HardwareArtwork kind="storage" />
      </div>
      <div className="ccc-storage-volumes">
        {overview?.volumes.length ? overview.volumes.map(volume => <div className="ccc-storage-volume" key={volume.mount}>
          <span className="ccc-storage-mount ccc-mono">{volume.mount}</span>
          <CapacityUsage capacity={volume.capacity} label={volume.mount} />
        </div>) : <CapacityUsage capacity={null} label="存储" />}
      </div>
    </Surface>
  </div>;
}
