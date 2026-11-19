import type { CSSProperties, KeyboardEvent, PointerEvent } from "react";
import type { RgbColor } from "../../api/client";
import { hexOf, hsvOf, rgbOfHsv } from "../../lib/color";
import { ColorPicker } from "../ColorPicker";

type ColorControlProps = {
  label: string;
  color: RgbColor;
  disabled: boolean;
  onChange: (color: RgbColor) => void;
};
function hueAt(event: PointerEvent<HTMLDivElement>) {
  const { left, top, width, height } = event.currentTarget.getBoundingClientRect();
  const x = event.clientX - left - width / 2;
  const y = event.clientY - top - height / 2;
  // A drag across the center should keep the last hue until the pointer leaves it.
  if (Math.hypot(x, y) < width * 0.3) return null;
  return (Math.round(Math.atan2(x, -y) * 180 / Math.PI) + 360) % 360;
}

export function LightingHueWheel({ label, color, disabled, onChange }: ColorControlProps) {
  const hsv = hsvOf(color);
  const hue = hsv.s === 0 ? null : Math.round(hsv.h) % 360;
  const hex = hexOf(color);
  const pick = (event: PointerEvent<HTMLDivElement>) => {
    const next = hueAt(event);
    if (!disabled && next !== null) onChange(rgbOfHsv({ h: next, s: 1, v: 1 }));
  };
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = hue ?? 0;
    let next: number;
    switch (event.key) {
      case "Home": next = 0; break;
      case "End": next = 359; break;
      case "ArrowUp": case "ArrowRight": next = Math.min(359, current + 1); break;
      case "ArrowDown": case "ArrowLeft": next = Math.max(0, current - 1); break;
      default: return;
    }
    event.preventDefault();
    if (!disabled) onChange(rgbOfHsv({ h: next, s: 1, v: 1 }));
  };
  return <div className="ccc-lighting-color">
    <div className="ccc-lighting-wheel">
      <div className="ccc-lighting-hue" role="slider" aria-label="色相" tabIndex={disabled ? -1 : 0}
        aria-disabled={disabled} aria-valuemin={0} aria-valuemax={359} aria-valuenow={hue ?? 0}
        aria-valuetext={hue === null ? "无色相" : `${hue} 度`} onKeyDown={keyDown}
        onPointerDown={event => {
          if (disabled || event.button !== 0 || !event.isPrimary) return;
          event.currentTarget.focus();
          event.currentTarget.setPointerCapture(event.pointerId);
          pick(event);
        }}
        onPointerMove={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) pick(event); }}>
        {hue !== null && <span className="ccc-lighting-hue-marker" aria-hidden="true"
          style={{ "--hue-angle": `${hue}deg`, "--hue-color": hex } as CSSProperties} />}
      </div>
      <div className="ccc-lighting-wheel-center">
        <ColorPicker key={label} label={label} value={color} disabled={disabled} onChange={onChange} swatchOnly />
      </div>
    </div>
    <div className="ccc-lighting-color-caption">
      <span>{label}</span><span className="ccc-mono">{hex.toUpperCase()}</span>
    </div>
  </div>;
}
