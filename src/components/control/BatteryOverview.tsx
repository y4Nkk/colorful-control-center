import type { BatteryProfile } from "../../api/client";
import { BatteryArtwork } from "./BatteryArtwork";

export function BatteryOverview({ profile }: { profile: BatteryProfile | null }) {
  const power = profile?.power;
  const present = power?.present === true;
  const percent = present ? power.percent : null;
  const reading = percent !== null && Number.isFinite(percent) && percent >= 0 && percent <= 100 ? percent : null;
  const charging = present ? power.charging : null;
  const full = power?.acOnline === true && reading === 100 && charging === false;
  const chargeState = charging === true ? "charging" : full ? "full" : undefined;
  const supply = power?.acOnline === true ? "外接电源" : power?.acOnline === false ? "电池供电" : present ? "供电未知" : null;
  const chargeStatus = present ? charging === null ? "充电状态未知" : charging ? "正在充电" : "未在充电" : null;
  const status = [supply, chargeStatus].filter(Boolean).join(" · ");
  const seconds = present ? power.remainingSeconds : null;
  const remaining = seconds !== null && Number.isFinite(seconds) && seconds >= 0 ? Math.ceil(seconds / 60) : null;

  return <section className="ccc-battery-overview" aria-label="当前电池读数" data-charge-state={chargeState}>
    <BatteryArtwork percent={reading} charging={charging} />
    <div className="ccc-battery-readout">
      {reading === null ? <strong className="ccc-battery-unavailable">{power?.present === false ? "未检测到电池" : "暂无电量读数"}</strong>
        : <div className="ccc-battery-level-value ccc-mono" role="meter" aria-label="当前电量"
          aria-valuemin={0} aria-valuemax={100} aria-valuenow={reading} aria-valuetext={`${reading}%`}>
          <span aria-hidden="true">{reading}<small>%</small></span>
        </div>}
      {status && <p className="ccc-control-caption">{status}</p>}
      {remaining !== null && <p className="ccc-control-caption">预计剩余 {remaining} 分钟</p>}
    </div>
  </section>;
}
