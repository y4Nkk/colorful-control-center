import { FAN_LABELS, type FanId, type HardwareSnapshot } from "../../api/client";
import { fmtInt } from "./format";

export function fanTemperature(live: HardwareSnapshot | null, id: FanId | undefined) {
  if (id === "cpu") return live?.cpuTempC ?? null;
  if (id === "gpu1") return live?.nvidia?.tempC ?? null;
  return null;
}

export function FanLiveReadings({ live, id, control }: {
  live: HardwareSnapshot | null; id?: FanId; control: string;
}) {
  const rpm = id === "cpu" ? live?.fan.cpuRpm : id === "gpu1" ? live?.fan.gpuRpm : null;
  const temp = fanTemperature(live, id);
  return <div className="ccc-fan-current">
    <div className="ccc-fan-current-head">
      <h3>{id ? `${FAN_LABELS[id]} 实况` : "风扇实况"}</h3>
      <span className="ccc-fan-control-state" role="status" aria-label="当前控制">{control}</span>
    </div>
    {id && <dl className="ccc-fan-readings">
      <div><dt>温度</dt><dd>{temp == null ? "—" : <>{fmtInt(temp)} <span>°C</span></>}</dd></div>
      <div><dt>实测转速</dt><dd>{rpm == null ? "—" : <>{fmtInt(rpm)} <span>RPM</span></>}</dd></div>
    </dl>}
  </div>;
}
