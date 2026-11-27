import { FAN_LABELS, type FanId, type RpmFan } from "../../api/client";
import { useHardwareHistory, useHardwareStatus } from "../../app/useHardwareHistory";
import { Button } from "../Button";
import type { FanCurveSelection } from "./FanCurveSection";
import { FanSpeedGauges } from "./FanSpeedGauges";
import { fmtInt } from "./format";

const fanIds = Object.keys(FAN_LABELS) as FanId[];

export function FanLiveReadings({ active, ids, ratedFans, selection }: {
  active: boolean; ids: FanId[]; ratedFans: readonly RpmFan[]; selection?: FanCurveSelection;
}) {
  const { live, samples } = useHardwareHistory(active);
  const { stale } = useHardwareStatus(active);
  if (!selection) {
    const visibleIds = fanIds.filter(id => ids.includes(id) ||
      samples.some(sample => sample.fan[id]?.rpm != null || sample.fan[id]?.tempC != null));
    return <FanSpeedGauges ids={visibleIds} live={live} ratedFans={ratedFans} samples={samples} stale={stale} />;
  }
  return ids.length > 0 && <div className="ccc-fan-readout-grid">
    {ids.map(id => {
      const reading = live?.fan[id];
      const rpm = reading?.rpm, temp = reading?.tempC;
      const name = FAN_LABELS[id];
      return <div key={id} className="ccc-fan-current" data-fan={id} role="group" aria-label={`${name} 实况`}>
        <h4><Button variant="ghost" size="compact" aria-label={`调节 ${name} 风扇曲线`} aria-controls={selection.fieldId(id)}
          aria-pressed={selection.frontFan === id} onClick={() => selection.chooseFan(id)}>
          <span className="ccc-fan-name">{name}</span>
        </Button></h4>
        <dl className="ccc-fan-readings">
          <div><dt>{stale && temp != null ? "上次温度" : "温度"}</dt><dd>{temp == null ? "—" : <>{fmtInt(temp)} <span>°C</span></>}</dd></div>
          <div><dt>{stale && rpm != null ? "上次转速" : "实测转速"}</dt><dd>{rpm == null ? "—" : <>{fmtInt(rpm)} <span>RPM</span></>}</dd></div>
        </dl>
      </div>;
    })}
  </div>;
}
