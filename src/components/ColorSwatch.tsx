import type { RgbColor } from "../api/client";
import { hexOf } from "../lib/color";
import { Tooltip } from "./Tooltip";

export function ColorSwatch({ color, label, className, fill, dimmed }: {
  color: RgbColor; label?: string; className?: string; fill?: boolean; dimmed?: boolean;
}) {
  const hex = hexOf(color);
  const swatch = <svg className={["ccc-color-swatch", className].filter(Boolean).join(" ")}
    data-fill={fill || undefined} data-dimmed={dimmed || undefined} viewBox="0 0 24 24" preserveAspectRatio="none" focusable="false"
    role={label ? "img" : undefined} aria-hidden={label ? undefined : true} aria-label={label}
    aria-description={label ? hex.toUpperCase() : undefined}>
    <rect width="24" height="24" fill={hex} />
  </svg>;
  return label ? <Tooltip content={hex.toUpperCase()}>{swatch}</Tooltip> : swatch;
}
