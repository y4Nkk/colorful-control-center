import { FAN_LABELS, type FanId, type HardwareSnapshot } from "../../api/client";
import { fmtInt } from "./format";

export function fanTemperature(live: HardwareSnapshot | null, id: FanId) {
  return live?.fan[id].tempC ?? null;
}

export function FanLiveReadings({ live, stale, id, control }: {
  live: HardwareSnapshot | null; stale: boolean; id?: FanId; control: string;
}) {
  const reading = id ? live?.fan[id] : null;
  const rpm = reading?.rpm, temp = reading?.tempC;
  return <div className="ccc-fan-current">
    <div className="ccc-fan-current-head">
      <h3>{id ? `${FAN_LABELS[id]} 实况` : "风扇实况"}</h3>
      <span className="ccc-fan-control-state" role="status" aria-label="当前控制">{control}</span>
    </div>
    {id && <dl className="ccc-fan-readings">
      <div><dt>{stale && temp != null ? "上次温度" : "温度"}</dt><dd>{temp == null ? "—" : <>{fmtInt(temp)} <span>°C</span></>}</dd></div>
      <div><dt>{stale && rpm != null ? "上次转速" : "实测转速"}</dt><dd>{rpm == null ? "—" : <>{fmtInt(rpm)} <span>RPM</span></>}</dd></div>
    </dl>}
  </div>;
}
