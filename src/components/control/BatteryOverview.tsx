import type { BatteryProfile } from "../../api/client";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { BatteryArtwork } from "./BatteryArtwork";
import { MetricReadout } from "./MetricReadout";

const validPercent = (value: number | null | undefined) => value != null && Number.isFinite(value) && value >= 0 && value <= 100 ? value : null;
export function BatteryLevel({ percent, charging, label = "当前电量" }: { percent: number | null; charging: boolean | null; label?: string }) {
  const reading = validPercent(percent);
  return <div className="ccc-battery-level" data-charging={charging === true} role={reading === null ? "img" : "meter"}
    aria-label={reading === null ? `${label}：暂无读数` : label} aria-valuemin={reading === null ? undefined : 0}
    aria-valuemax={reading === null ? undefined : 100} aria-valuenow={reading ?? undefined} aria-valuetext={reading === null ? undefined : `${reading}%`}>
    <BatteryArtwork percent={reading} charging={charging} />
    <span className="ccc-battery-level-value ccc-mono" aria-hidden="true">{reading ?? "—"}{reading !== null && <em>%</em>}</span>
  </div>;
}
export function BatteryOverview({ profile }: { profile: BatteryProfile | null }) {
  const power = profile?.power, present = power?.present === true;
  const diagnostics = present ? profile?.diagnostics : null;
  const supply = power?.acOnline == null ? "供电未知" : power.acOnline ? "外接电源" : "电池供电";
  const charging = !present ? null : power?.charging ?? null;
  const status = power?.present === false ? "未检测到电池" : !present || charging === null ? "电池状态未知" : charging ? "正在充电" : "未在充电";
  const remaining = present && power?.remainingSeconds != null ? `${Math.ceil(power.remainingSeconds / 60)} 分钟` : "—";
  return <div className="ccc-battery-overview" aria-label="电池实时概览">
    <Surface as="section" variant="inset" className="ccc-battery-overview-part" aria-label="当前电池读数">
      <div className="ccc-battery-overview-head"><span><Icon name={power?.acOnline === true ? "plug" : "battery"} />{supply}</span><span>{status}</span></div>
      <BatteryLevel percent={present ? power?.percent ?? null : null} charging={charging} />
      <div className="ccc-battery-overview-foot"><span>当前电量</span><span>预计剩余 <strong>{remaining}</strong></span></div>
    </Surface>
    <Surface as="section" variant="inset" className="ccc-battery-overview-part" aria-label="电池容量读数">
      <div className="ccc-battery-overview-head"><span><Icon name="battery" />容量对比</span><span>原厂报告</span></div>
      <div className="ccc-battery-capacity"><MetricReadout label="满充 / 设计容量" value={diagnostics?.capacityPct ?? null} maximum={100} unit="%" colorVar="var(--ccc-safe)" />
        <div className="ccc-battery-cycle"><Icon name="refresh" /><strong className="ccc-mono">{diagnostics?.cycleCount ?? "—"}<small> 次</small></strong><span>累计循环</span></div>
      </div>
      <div className="ccc-battery-overview-foot"><span>生产日期</span><strong>{diagnostics?.manufacturedOn ?? "—"}</strong></div>
    </Surface>
  </div>;
}
