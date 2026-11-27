import { Surface } from "../Surface";
import type { HardwareSnapshot } from "../../api/client";
import type { MemorySample, SystemOverview } from "../../app/useSystemOverview";
import { HardwareCardHeader } from "./HardwareCardHeader";
import { CapacityOverview } from "./CapacityOverview";
import { CpuParameters, GpuParameters } from "./HardwareParameters";
import { ProcessorReadings } from "./ProcessorReadings";

export function HardwareReadings({ live, samples, overview, memoryHistory, retainCpuFrequency }: {
  live: HardwareSnapshot | null; samples: readonly HardwareSnapshot[];
  overview: SystemOverview; memoryHistory: MemorySample[]; retainCpuFrequency: boolean;
}) {
  const cpuIdentity = overview.inventory?.cpu;
  const vendor = cpuIdentity?.vendor;
  const cpuBrand = vendor === "intel" || vendor === "amd" ? vendor : null;
  return <div className="ccc-module-grid">
    <div className="ccc-module-grid" data-module-layout="readings">
      <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="CPU 运行读数">
        <HardwareCardHeader kind="cpu" brand={cpuBrand} title="CPU" model={cpuIdentity?.name ?? "处理器型号未识别"}>
          <CpuParameters cpu={cpuIdentity} live={live} />
        </HardwareCardHeader>
        <ProcessorReadings part="CPU" live={live} samples={samples} retainFrequency={retainCpuFrequency} />
      </Surface>
      <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="GPU 运行读数">
        <HardwareCardHeader kind="gpu" brand={live?.nvidia ? "nvidia" : null} title="GPU" model={live?.nvidia?.name || "显卡型号未识别"}>
          <GpuParameters live={live} />
        </HardwareCardHeader>
        <ProcessorReadings part="GPU" live={live} samples={samples} />
      </Surface>
    </div>
    <CapacityOverview inventory={overview.inventory} capacity={overview.memory} memoryHistory={memoryHistory}
      reportedClockMhz={live?.oemMemory?.reportedClockMhz ?? null} referenceClockMhz={live?.oemMemory?.referenceClockMhz ?? null} />
  </div>;
}
