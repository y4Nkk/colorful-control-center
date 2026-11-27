import type { ReactNode } from "react";
import { fmtInt } from "./format";
import { capacityParts, formatCapacity, formatParameter } from "../../lib/format-parameters";

const ratioPaths = {
  ring: "M40 5 A35 35 0 1 1 40 75 A35 35 0 1 1 40 5 Z",
  arc: "M15.251 64.749 A35 35 0 1 1 64.749 64.749",
};

interface MetricReadoutProps {
  label: string;
  value: number | null;
  unit: "RPM" | "%" | "bytes" | "MHz" | "°C" | "MT/s";
  maximum?: number | null;
  colorVar?: string;
  ratioShape?: keyof typeof ratioPaths;
  plot?: ReactNode;
  compact?: boolean;
  ariaLabel?: string;
  emptyText?: string;
  alertAt?: number;
}

/** 只有读数和分母均有效时才填充比例图。 */
export function MetricReadout({ label, value, unit, maximum, colorVar = "var(--ccc-chart-blue)", plot,
  compact = false, ratioShape = "ring", ariaLabel = label, emptyText = "—", alertAt }: MetricReadoutProps) {
  const isCapacity = unit === "bytes";
  const reading = value != null && Number.isFinite(value) && (!isCapacity || value >= 0) ? value : null;
  const isRatio = maximum !== undefined;
  const limit = maximum != null && Number.isFinite(maximum) && maximum > 0 ? maximum : null;
  const measured = reading != null && limit != null ? Math.min(limit, Math.max(0, reading)) : null;
  const ratio = measured != null && limit != null ? measured / limit * 100 : null;
  const display = reading == null ? null : isCapacity ? capacityParts(reading)
    : { value: unit === "°C" ? formatParameter(reading, "", 1) : fmtInt(reading), unit };
  const current = display == null ? "暂无读数" : `${display.value} ${display.unit}`;
  const total = limit == null ? null : isCapacity ? formatCapacity(limit) : `${fmtInt(limit)} ${unit}`;
  const meter = {
    role: measured == null ? "img" : "meter",
    "aria-label": measured == null ? `${ariaLabel}：${current}${display && total == null ? "，总量未知" : ""}` : ariaLabel,
    "aria-valuemin": measured == null ? undefined : 0,
    "aria-valuemax": measured == null ? undefined : limit ?? undefined,
    "aria-valuenow": measured ?? undefined,
    "aria-valuetext": measured == null ? undefined : `${current} / ${total}`,
  };
  const text = <output className="ccc-mono ccc-meter-value" aria-label={compact && !isRatio ? ariaLabel : undefined}
    data-alert={alertAt != null && reading != null && reading >= alertAt || undefined}>
    <span>{display?.value ?? emptyText}</span>
    <span className="ccc-meter-unit">{display && ` ${display.unit}`}</span>
  </output>;
  const caption = <span className="ccc-meter-label">{label}</span>;
  const path = ratioPaths[ratioShape];
  const ratioGraphic = isRatio && <svg viewBox={compact ? "-6 -6 92 92" : "0 0 80 80"}
    fill="none" strokeLinecap="round" aria-hidden="true" focusable="false">
    <path className="ccc-meter-ring-track" d={path} />
    {ratio != null && ratio > 0 && <path className="ccc-meter-ring-fill" d={path}
      pathLength="100" strokeDasharray={`${ratio} 100`} />}
  </svg>;
  if (compact) return <div className="ccc-meter ccc-meter-compact"
    style={isRatio ? { color: colorVar } : undefined} {...(isRatio ? meter : {})}>
    {plot ?? ratioGraphic}
    {caption}
    {text}
  </div>;
  const captionInside = isRatio && ratioShape === "arc";
  return <div className={`ccc-meter${isRatio ? " is-ratio" : ""}`}>
    {isRatio ? <div className="ccc-meter-ring" style={{ color: colorVar }} {...meter}>
      {ratioGraphic}
      {captionInside ? <div className="ccc-meter-ring-content">{text}{caption}</div> : text}
    </div> : text}
    {!captionInside && caption}
  </div>;
}
