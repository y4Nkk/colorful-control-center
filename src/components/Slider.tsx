import type { ComponentProps, CSSProperties } from "react";

interface SliderProps extends Omit<ComponentProps<"input">, "type" | "min" | "max" | "step" | "value" | "defaultValue"> {
  min: number;
  max: number;
  step?: number;
  value: number;
  appearance?: "hue";
  marker?: { value: number; label: string };
}

function progress(value: number, min: number, max: number) {
  if (max <= min || !Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, (value - min) / (max - min)));
}

function position(ratio: number) {
  return `calc(${ratio * 100}% + ${0.5 - ratio} * var(--ccc-range-thumb))`;
}

export function Slider({ className, appearance, min, max, step = 1, value, marker, disabled, ...props }: SliderProps) {
  const ratio = progress(value, min, max);
  const thumb = position(ratio);
  const style = {
    "--range-position": thumb,
    "--range-fill": ratio === 1 ? "100%" : thumb,
  } as CSSProperties;
  const stops = Math.floor((max - min) / step) + 1;
  const ticks = appearance === "hue" || max <= min ? 0 : stops > 13 ? 7 : Math.max(2, stops);

  return <div className="ccc-range" data-appearance={appearance} data-disabled={disabled || undefined} style={style}>
    <div className="ccc-range-track" aria-hidden="true">
      {appearance !== "hue" && <span className="ccc-range-fill" />}
      {ticks > 0 && <span className="ccc-range-ticks">
        {Array.from({ length: ticks }, (_, index) => {
          const stop = index / (ticks - 1);
          return <span key={index} className="ccc-range-tick" data-filled={stop <= ratio + 1e-6}
            style={{ left: `${stop * 100}%` }} />;
        })}
      </span>}
    </div>
    {marker && <span className="ccc-range-marker" style={{ left: position(progress(marker.value, min, max)) }}
      aria-hidden="true" title={marker.label} />}
    <span className="ccc-range-thumb" aria-hidden="true" />
    <input {...props} type="range" min={min} max={max} step={step} value={value} disabled={disabled}
      className={["ccc-range-input", className].filter(Boolean).join(" ")} />
  </div>;
}
