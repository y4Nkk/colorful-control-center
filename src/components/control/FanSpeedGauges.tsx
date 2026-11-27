import { FAN_LABELS, type FanId, type HardwareSnapshot, type RpmFan } from "../../api/client";
import { formatParameter } from "../../lib/format-parameters";
import { fmtInt } from "./format";

const RPM_PER_THOUSAND = 1_000;
const MINIMUM_SCALE_RPM = 4_000;
const CENTER_X = 90;
const CENTER_Y = 82;
const START_ANGLE = -225;
const SWEEP = 270;
const RADIUS = 61;

function dialPoint(angle: number, radius: number) {
  const radians = angle * Math.PI / 180;
  return [
    Number((CENTER_X + radius * Math.cos(radians)).toFixed(2)),
    Number((CENTER_Y + radius * Math.sin(radians)).toFixed(2)),
  ] as const;
}

const dialTrack = `M${dialPoint(START_ANGLE, RADIUS).join(" ")} A${RADIUS} ${RADIUS} 0 1 1 ${dialPoint(START_ANGLE + SWEEP, RADIUS).join(" ")}`;

function validRpm(value: number | null | undefined): value is number {
  return value != null && Number.isFinite(value) && value >= 0;
}

function speedScale(ids: readonly FanId[], ratedFans: readonly RpmFan[], samples: readonly HardwareSnapshot[]) {
  let highest = MINIMUM_SCALE_RPM;
  for (const fan of ratedFans) {
    if (ids.includes(fan.id) && validRpm(fan.maximumRpm)) highest = Math.max(highest, fan.maximumRpm);
  }
  for (const sample of samples) {
    for (const id of ids) {
      const rpm = sample.fan[id]?.rpm;
      if (validRpm(rpm)) highest = Math.max(highest, rpm);
    }
  }
  return Math.ceil(highest / RPM_PER_THOUSAND);
}

function DialTicks({ maximum }: { maximum: number }) {
  const labelStep = Math.ceil(maximum / 10);
  const divisions = labelStep === 1 ? 4 : 2;
  const interval = labelStep / divisions;
  const marks = Array.from({ length: Math.ceil(maximum / interval) }, (_, index) => index * interval);
  return <g>{[...marks, maximum].map((value, index) => {
    const major = value === maximum || index % divisions === 0;
    const angle = START_ANGLE + SWEEP * value / maximum;
    const [x1, y1] = dialPoint(angle, major ? 66 : 68);
    const [x2, y2] = dialPoint(angle, 71);
    const [x, y] = dialPoint(angle, 80);
    return <g key={value}>
      <line className="ccc-fan-dial-tick" data-major={major || undefined} x1={x1} y1={y1} x2={x2} y2={y2} />
      {major && <text className="ccc-fan-dial-scale" x={x} y={y}>{value}</text>}
    </g>;
  })}</g>;
}

function FanSpeedDial({ name, rpm, maximum, stale }: {
  name: string;
  rpm: number | null;
  maximum: number;
  stale: boolean;
}) {
  const measured = rpm !== null;
  const speed = measured ? rpm / RPM_PER_THOUSAND : 0;
  const portion = speed / maximum;
  const value = measured ? formatParameter(speed, "", 3) : "—";
  return <div className="ccc-fan-dial" role={measured ? "meter" : "img"}
    aria-label={measured ? `${name} ${stale ? "上次转速" : "实测转速"}` : `${name} 转速未读取`}
    aria-valuemin={measured ? 0 : undefined} aria-valuemax={measured ? maximum : undefined}
    aria-valuenow={measured ? speed : undefined} aria-valuetext={measured ? `${fmtInt(rpm)} RPM` : undefined}
    data-stale={stale && measured || undefined}>
    <svg viewBox="0 -8 180 156" aria-hidden="true" focusable="false">
      <path className="ccc-fan-dial-track" d={dialTrack} />
      {portion > 0 && <path className="ccc-fan-dial-fill" d={dialTrack}
        pathLength="100" strokeDasharray={`${portion * 100} 100`} />}
      <DialTicks maximum={maximum} />
      {measured && <>
        <path className="ccc-fan-dial-needle" d="M82 80.2 142 82 82 83.8Z"
          transform={`rotate(${START_ANGLE + SWEEP * portion} ${CENTER_X} ${CENTER_Y})`} />
        <circle className="ccc-fan-dial-pivot" cx={CENTER_X} cy={CENTER_Y} r="4" />
      </>}
      <text className="ccc-fan-dial-value" x={CENTER_X} y="126">{value}</text>
    </svg>
  </div>;
}

export function FanSpeedGauges({ ids, live, ratedFans, samples, stale }: {
  ids: readonly FanId[];
  live: HardwareSnapshot | null;
  ratedFans: readonly RpmFan[];
  samples: readonly HardwareSnapshot[];
  stale: boolean;
}) {
  const maximum = speedScale(ids, ratedFans, samples);
  return <div className="ccc-fan-speed">
    <h3>风扇转速</h3>
    {ids.length ? <div className="ccc-fan-speed-grid">
      {ids.map(id => {
        const name = FAN_LABELS[id], reading = live?.fan[id];
        const rpm = validRpm(reading?.rpm) ? reading.rpm : null;
        const temp = reading?.tempC ?? null;
        return <div key={id} className="ccc-fan-speed-card" data-fan={id} role="group" aria-label={`${name} 实况`}>
          <div className="ccc-fan-speed-heading">
            <h4 className="ccc-fan-name">{name}</h4>
            {stale && rpm !== null && <span className="ccc-control-caption">上次转速</span>}
          </div>
          <FanSpeedDial name={name} rpm={rpm} maximum={maximum} stale={stale} />
          <dl className="ccc-fan-speed-temperature">
            <dt>{stale && temp !== null ? "上次温度" : "温度"}</dt>
            <dd className="ccc-mono">{temp === null ? "—" : <>{fmtInt(temp)} <span>°C</span></>}</dd>
          </dl>
        </div>;
      })}
    </div> : <p className="ccc-control-caption">{samples.length ? "暂无转速读数" : "等待采样"}</p>}
  </div>;
}
