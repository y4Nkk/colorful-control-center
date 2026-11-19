import { fmtInt } from "./format";
import { capacityParts, formatCapacity } from "../../lib/format-parameters";

interface MetricReadoutProps {
  label: string;
  value: number | null;
  unit: "RPM" | "%" | "bytes";
  maximum?: number | null;
  colorVar: string;
}

/** 未知分母保留空环；RPM 不提供分母，不伪造可比较的转速比例。 */
export function MetricReadout({ label, value, unit, maximum, colorVar }: MetricReadoutProps) {
  const isCapacity = unit === "bytes";
  const reading = value != null && Number.isFinite(value) && (!isCapacity || value >= 0) ? value : null;
  const isRatio = maximum !== undefined;
  const limit = maximum != null && Number.isFinite(maximum) && maximum > 0 ? maximum : null;
  const measured = reading != null && limit != null ? Math.min(limit, Math.max(0, reading)) : null;
  const ratio = measured != null && limit != null ? measured / limit * 100 : null;
  const display = reading == null ? null : isCapacity ? capacityParts(reading) : { value: fmtInt(reading), unit };
  const current = display == null ? "暂无读数" : `${display.value} ${display.unit}`;
  const total = limit == null ? null : isCapacity ? formatCapacity(limit) : `${fmtInt(limit)} ${unit}`;
  const text = <output className="ccc-mono ccc-meter-value">
    <span>{display?.value ?? "—"}</span>
    <span className="ccc-meter-unit">{display && ` ${display.unit}`}</span>
  </output>;
  return <div className={`ccc-meter${isRatio ? " is-ratio" : ""}`}>
    {isRatio ? <div className="ccc-meter-ring" style={{ color: colorVar }}
      role={measured == null ? "img" : "meter"}
      aria-label={measured == null ? `${label}：${current}${display && total == null ? "，总量未知" : ""}` : label}
      aria-valuemin={measured == null ? undefined : 0} aria-valuemax={measured == null ? undefined : limit!}
      aria-valuenow={measured ?? undefined}
      aria-valuetext={measured == null ? undefined : `${current} / ${total}`}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle className="ccc-meter-ring-track" cx="40" cy="40" r="35" />
        {ratio != null && ratio > 0 && <circle className="ccc-meter-ring-fill" cx="40" cy="40" r="35"
          pathLength="100" strokeDasharray={`${ratio} 100`} transform="rotate(-90 40 40)" />}
      </svg>
      {text}
    </div> : text}
    <span className="ccc-meter-label">{label}</span>
    {isCapacity && <span className="ccc-meter-total">{total == null ? "总容量未知" : `共 ${total}`}</span>}
  </div>;
}
