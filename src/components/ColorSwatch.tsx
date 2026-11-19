import type { RgbColor } from "../api/client";
import { hexOf } from "../lib/color";

export function ColorSwatch({ color, label, className, fill, dimmed }: {
  color: RgbColor; label?: string; className?: string; fill?: boolean; dimmed?: boolean;
}) {
  const hex = hexOf(color);
  return <span className={["ccc-color-swatch", className].filter(Boolean).join(" ")}
    data-fill={fill || undefined} data-dimmed={dimmed || undefined} style={{ backgroundColor: hex }}
    role={label ? "img" : undefined} aria-hidden={label ? undefined : true} aria-label={label}
    aria-description={label ? hex.toUpperCase() : undefined} title={label ? hex.toUpperCase() : undefined} />;
}
