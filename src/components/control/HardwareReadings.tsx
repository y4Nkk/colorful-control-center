import { Surface } from "../Surface";
import { Button } from "../Button";
import type { HardwareSnapshot } from "../../api/client";
import { MetricReadout } from "./MetricReadout";
import { useSystemOverview } from "../../app/useSystemOverview";
import { BrandMark, HardwareArtwork } from "./HardwareArtwork";
import { CapacityOverview } from "./CapacityOverview";
import { Collapsible } from "../Collapsible";
import { CpuParameters, GpuParameters } from "./HardwareParameters";

export function HardwareReadings({ live, active }: { live: HardwareSnapshot | null; active: boolean }) {
  const cpu = "var(--ccc-action)", gpu = "var(--ccc-safe)";
  const utility = live?.cpuUtility;
  const cpuUsage = utility && live.sampledAtMs >= utility.sampledAtMs && live.sampledAtMs - utility.sampledAtMs <= 2500 ? utility.valuePct : null;
  const memory = live?.nvidia?.memUsedMb, total = live?.nvidia?.memTotalMb;
  const { overview, error, reading, retry } = useSystemOverview(active);
  const brand = overview?.cpu.vendor;
  return <div className="ccc-hardware-overview">
    <div className="ccc-hardware-readings">
    <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="CPU 散热读数">
      <div className="ccc-sensor-identity">
        <div className="ccc-hardware-description">
          <BrandMark brand={brand === "intel" || brand === "amd" ? brand : null} />
          <h3>CPU <span>处理器</span></h3>
          <p>{overview?.cpu.name ?? "处理器型号未识别"}</p>
        </div>
        <HardwareArtwork kind="cpu" brand={brand === "intel" || brand === "amd" ? brand : null} />
      </div>
      <div className="ccc-sensor-metrics">
      <div className="ccc-sensor-summary">
        <MetricReadout label="风扇转速" value={live?.fan.cpuRpm ?? null} unit="RPM" colorVar={cpu} />
      </div>
      <div className="ccc-sensor-ratios">
        <MetricReadout label="风扇占空比" value={live?.fan.cpuDutyPct ?? null} unit="%" maximum={100} colorVar={cpu} />
        <MetricReadout label="CPU 利用率" value={cpuUsage} unit="%" maximum={100} colorVar={cpu} />
      </div>
      </div>
      <CpuParameters overview={overview} live={live} />
    </Surface>
    <Surface as="section" variant="card" className="ccc-sensor-group" aria-label="GPU 运行读数">
      <div className="ccc-sensor-identity">
        <div className="ccc-hardware-description">
          <BrandMark brand={live?.nvidia ? "nvidia" : null} />
          <h3>GPU <span>图形处理器</span></h3>
          <p>{live?.nvidia?.name || "显卡型号未识别"}</p>
        </div>
        <HardwareArtwork kind="gpu" />
      </div>
      <div className="ccc-sensor-metrics">
      <div className="ccc-sensor-summary">
        <MetricReadout label="风扇转速" value={live?.fan.gpuRpm ?? null} unit="RPM" colorVar={gpu} />
      </div>
      <div className="ccc-sensor-ratios">
        <MetricReadout label="风扇占空比" value={live?.fan.gpuDutyPct ?? null} unit="%" maximum={100} colorVar={gpu} />
        <MetricReadout label="利用率" value={live?.nvidia?.utilizationPct ?? null} unit="%" maximum={100} colorVar={gpu} />
        <MetricReadout label="显存占用" value={memory == null ? null : memory / 1024}
          maximum={total == null ? null : total / 1024} format={value => value.toFixed(1)} unit="GiB" colorVar={gpu} />
      </div>
      </div>
      <GpuParameters live={live} />
    </Surface>
    </div>
    <CapacityOverview overview={overview} oemMemory={live?.oemMemory ?? null} />
    {(error || (overview?.warnings.length ?? 0) > 0) && <Collapsible summary="概览读取异常">
      {error && <p className="ccc-control-error">{error}</p>}
      {overview?.warnings.map(warning => <p key={warning}>{warning}</p>)}
      <Button variant="ghost" size="compact" type="button" onClick={retry} disabled={reading} aria-busy={reading}>
        {reading ? "读取中…" : "重新读取"}
      </Button>
    </Collapsible>}
  </div>;
}
