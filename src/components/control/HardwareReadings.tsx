import { Surface } from "../Surface";
import type { HardwareSnapshot, SystemOverview } from "../../api/client";
import { MetricReadout } from "./MetricReadout";
import { SensorTemperature } from "./SensorTemperature";
import type { MemorySample } from "../../app/useSystemOverview";
import { BrandMark, HardwareArtwork } from "./HardwareArtwork";
import { CapacityOverview } from "./CapacityOverview";
import { CpuParameters, GpuParameters } from "./HardwareParameters";
import { BYTES_PER_MB } from "../../lib/format-parameters";

export function HardwareReadings({ live, samples, overview, memoryHistory }: {
  live: HardwareSnapshot | null; samples: readonly HardwareSnapshot[];
  overview: SystemOverview | null; memoryHistory: MemorySample[];
}) {
  const cpu = "var(--ccc-action)", gpu = "var(--ccc-safe)";
  const utility = live?.cpuUtility;
  const cpuUsage = utility && live.sampledAtMs >= utility.sampledAtMs && live.sampledAtMs - utility.sampledAtMs <= 2500 ? utility.valuePct : null;
  const memory = live?.nvidia?.memUsedMb, total = live?.nvidia?.memTotalMb;
  const vendor = overview?.cpu.vendor;
  const cpuBrand = vendor === "intel" || vendor === "amd" ? vendor : null;
  return <div className="ccc-module-grid">
    <div className="ccc-module-grid" data-module-layout="readings">
      <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="CPU 运行读数">
        <div className="ccc-sensor-identity">
          <div className="ccc-hardware-description">
            <BrandMark brand={cpuBrand} />
            <h3>CPU <span>处理器</span></h3>
            <p>{overview?.cpu.name ?? "处理器型号未识别"}</p>
          </div>
          <div className="ccc-sensor-visual">
            <HardwareArtwork kind="cpu" brand={cpuBrand} />
            <CpuParameters overview={overview} live={live} />
          </div>
        </div>
        <div className="ccc-sensor-metrics">
          <div className="ccc-sensor-summary">
            <MetricReadout label="当前频率" value={utility?.clockMhz ?? null} unit="MHz" colorVar={cpu} />
          </div>
          <div className="ccc-sensor-ratios">
            <MetricReadout label="风扇占空比" value={live?.fan.cpuDutyPct ?? null} unit="%" maximum={100} colorVar={cpu} />
            <MetricReadout label="CPU 利用率" value={cpuUsage} unit="%" maximum={100} colorVar={cpu} />
          </div>
        </div>
        <SensorTemperature label="CPU" samples={samples} read={sample => sample.cpuTempC} colorVar={cpu} />
      </Surface>
      <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="GPU 运行读数">
        <div className="ccc-sensor-identity">
          <div className="ccc-hardware-description">
            <BrandMark brand={live?.nvidia ? "nvidia" : null} />
            <h3>GPU <span>图形处理器</span></h3>
            <p>{live?.nvidia?.name || "显卡型号未识别"}</p>
          </div>
          <div className="ccc-sensor-visual">
            <HardwareArtwork kind="gpu" />
            <GpuParameters live={live} />
          </div>
        </div>
        <div className="ccc-sensor-metrics">
          <div className="ccc-sensor-summary">
            <MetricReadout label="当前频率" value={live?.nvidia?.graphicsClockMhz ?? null} unit="MHz" colorVar={gpu} />
          </div>
          <div className="ccc-sensor-ratios">
            <MetricReadout label="风扇占空比" value={live?.fan.gpuDutyPct ?? null} unit="%" maximum={100} colorVar={gpu} />
            <MetricReadout label="利用率" value={live?.nvidia?.utilizationPct ?? null} unit="%" maximum={100} colorVar={gpu} />
            <MetricReadout label="显存占用" value={memory == null ? null : memory * BYTES_PER_MB}
              maximum={total == null ? null : total * BYTES_PER_MB} unit="bytes" colorVar={gpu} />
          </div>
        </div>
        <SensorTemperature label="GPU" samples={samples} read={sample => sample.nvidia?.tempC ?? null} colorVar={gpu} />
      </Surface>
    </div>
    <CapacityOverview overview={overview} oemMemory={live?.oemMemory ?? null} memoryHistory={memoryHistory} />
  </div>;
}
