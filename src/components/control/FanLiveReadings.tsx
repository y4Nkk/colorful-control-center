import type { FanId } from "../../api/client";
import { useHardwareHistory, useHardwareStatus } from "../../app/useHardwareHistory";
import { fmtInt } from "./format";

export function FanLiveReadings({ active, id }: { active: boolean; id: FanId }) {
  const { live } = useHardwareHistory(active);
  const { stale } = useHardwareStatus(active);
  const reading = live?.fan[id];
  const rpm = reading?.rpm, temp = reading?.tempC;
  return <div className="ccc-fan-current">
    <dl className="ccc-fan-readings">
      <div><dt>{stale && temp != null ? "上次温度" : "温度"}</dt><dd>{temp == null ? "—" : <>{fmtInt(temp)} <span>°C</span></>}</dd></div>
      <div><dt>{stale && rpm != null ? "上次转速" : "实测转速"}</dt><dd>{rpm == null ? "—" : <>{fmtInt(rpm)} <span>RPM</span></>}</dd></div>
    </dl>
  </div>;
}
