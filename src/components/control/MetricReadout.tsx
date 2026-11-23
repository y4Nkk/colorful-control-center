import { fmtInt } from "./format";
import { capacityParts, formatCapacity } from "../../lib/format-parameters";
import { Icon, type IconName } from "../Icon";

interface MetricReadoutProps {
  label: string;
  value: number | null;
  unit: "RPM" | "%" | "bytes" | "MHz";
  maximum?: number | null;
  colorVar: string;
  icon?: IconName;
}

/** 只有读数和分母均有效时才填充比例图。 */
export function MetricReadout({ label, value, unit, maximum, colorVar, icon }: MetricReadoutProps) {
  const isCapacity = unit === "bytes";
  const reading = value != null && Number.isFinite(value) && (!isCapacity || value >= 0) ? value : null;
  const isRatio = maximum !== undefined;
  const limit = maximum != null && Number.isFinite(maximum) && maximum > 0 ? maximum : null;
  const measured = reading != null && limit != null ? Math.min(limit, Math.max(0, reading)) : null;
  const ratio = measured != null && limit != null ? measured / limit * 100 : null;
  const display = reading == null ? null : isCapacity ? capacityParts(reading) : { value: fmtInt(reading), unit };
  const current = display == null ? "暂无读数" : `${display.value} ${display.unit}`;
  const total = limit == null ? null : isCapacity ? formatCapacity(limit) : `${fmtInt(limit)} ${unit}`;
  const meter = {
    role: measured == null ? "img" : "meter",
    "aria-label": measured == null ? `${label}：${current}${display && total == null ? "，总量未知" : ""}` : label,
    "aria-valuemin": measured == null ? undefined : 0,
    "aria-valuemax": measured == null ? undefined : limit ?? undefined,
    "aria-valuenow": measured ?? undefined,
    "aria-valuetext": measured == null ? undefined : `${current} / ${total}`,
  };
  const text = <output className="ccc-mono ccc-meter-value" aria-label={icon && !isRatio ? label : undefined}>
    <span>{display?.value ?? "—"}</span>
    <span className="ccc-meter-unit">{display && ` ${display.unit}`}</span>
  </output>;
  if (icon) return <div className="ccc-meter ccc-meter-compact" {...(isRatio ? meter : {})}>
    <div className="ccc-meter-heading">
      <Icon name={icon} size={16} />
      <span className="ccc-meter-label">{label}</span>
    </div>
    {text}
  </div>;
  return <div className={`ccc-meter${isRatio ? " is-ratio" : ""}`}>
    {isRatio ? <div className="ccc-meter-ring" style={{ color: colorVar }} {...meter}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle className="ccc-meter-ring-track" cx="40" cy="40" r="35" />
        {ratio != null && ratio > 0 && <circle className="ccc-meter-ring-fill" cx="40" cy="40" r="35"
          pathLength="100" strokeDasharray={`${ratio} 100`} transform="rotate(-90 40 40)" />}
      </svg>
      {text}
    </div> : text}
    <span className="ccc-meter-label">{label}</span>
  </div>;
}
