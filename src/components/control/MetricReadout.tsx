import { fmtInt } from "./format";

interface MetricReadoutProps {
  label: string;
  value: number | null;
  unit?: string;
  maximum?: number | null;
  format?: (value: number) => string;
  colorVar: string;
}

/** 未知分母保留空环；RPM 不提供分母，不伪造可比较的转速比例。 */
export function MetricReadout({ label, value, unit = "", maximum, format = fmtInt, colorVar }: MetricReadoutProps) {
  const reading = value != null && Number.isFinite(value) ? value : null;
  const isRatio = maximum !== undefined;
  const limit = maximum != null && Number.isFinite(maximum) && maximum > 0 ? maximum : null;
  const measured = reading != null && limit != null ? Math.min(limit, Math.max(0, reading)) : null;
  const ratio = measured != null && limit != null ? measured / limit * 100 : null;
  const text = <output className="ccc-mono ccc-meter-value">
      {reading == null ? "—" : format(reading)}
      {reading != null && unit && <em>{` ${unit}`}</em>}
    </output>;
  return <div className={`ccc-meter${isRatio ? " is-ratio" : ""}`}>
    {isRatio ? <div className={`ccc-meter-ring${unit === "GiB" ? " is-capacity" : ""}`} style={{ color: colorVar }}
      role={measured == null ? "img" : "meter"}
      aria-label={measured == null ? `${label}：${reading == null ? "暂无读数" : `${format(reading)} ${unit}，总量未知`}` : label}
      aria-valuemin={measured == null ? undefined : 0} aria-valuemax={measured == null ? undefined : limit!}
      aria-valuenow={measured ?? undefined}
      aria-valuetext={measured == null ? undefined : `${format(reading!)} ${unit} / ${format(limit!)} ${unit}`}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle className="ccc-meter-ring-track" cx="40" cy="40" r="35" />
        {ratio != null && ratio > 0 && <circle className="ccc-meter-ring-fill" cx="40" cy="40" r="35"
          pathLength="100" strokeDasharray={`${ratio} 100`} transform="rotate(-90 40 40)" />}
      </svg>
      {text}
    </div> : text}
    <span className="ccc-meter-label">{label}</span>
    {isRatio && unit !== "%" && <span className="ccc-meter-total">{limit == null ? "总容量未知" : `共 ${format(limit)} ${unit}`}</span>}
  </div>;
}
