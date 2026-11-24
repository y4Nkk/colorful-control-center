import type { ComponentProps, CSSProperties } from "react";
import { useSliderInput } from "./use-slider-input";

interface SliderProps extends Pick<ComponentProps<"input">, "id" | "aria-label" | "aria-valuetext" | "disabled"> {
  min: number;
  max: number;
  value: number;
  busy?: boolean;
  appearance?: "hue";
  marker?: { value: number; label: string };
  magnetInterval?: number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  onCancel?: () => void;
}

function progress(value: number, min: number, max: number) {
  if (max <= min || !Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, (value - min) / (max - min)));
}

function position(ratio: number) {
  return `calc(${ratio * 100}% + ${0.5 - ratio} * var(--ccc-range-thumb))`;
}

function tickPositions(min: number, max: number, magnetInterval?: number) {
  if (max <= min) return [];
  if (magnetInterval) {
    const ticks = [0];
    for (let mark = Math.ceil((min + 1) / magnetInterval) * magnetInterval; mark < max; mark += magnetInterval) {
      ticks.push(progress(mark, min, max));
    }
    return [...ticks, 1];
  }
  const ticks = max - min >= 13 ? 7 : max - min + 1;
  return Array.from({ length: ticks }, (_, index) => index / (ticks - 1));
}

export function Slider({ appearance, min, max, value, marker, magnetInterval, disabled = false, busy = false,
  onChange, onCommit, onCancel, ...props }: SliderProps) {
  const { input, rail, dragging, pointer, keyboard } = useSliderInput({
    min, max, magnetInterval, disabled: disabled || busy, onChange, onCommit, onCancel,
  });
  const ratio = progress(value, min, max);
  const thumb = position(ratio);
  const style = {
    "--range-position": thumb,
    "--range-fill": ratio === 1 ? "100%" : thumb,
  } as CSSProperties;
  const ticks = appearance === "hue" ? [] : tickPositions(min, max, magnetInterval);

  return <div className="ccc-range" data-appearance={appearance} data-disabled={disabled || undefined}
    data-busy={busy && !disabled || undefined} data-dragging={dragging || undefined} style={style} {...pointer}>
    <div className="ccc-range-track" aria-hidden="true">
      {appearance !== "hue" && <span className="ccc-range-fill" />}
      <span ref={rail} className="ccc-range-ticks">
        {ticks.map(stop => <span key={stop} className="ccc-range-tick" data-filled={stop <= ratio + 1e-6}
          style={{ left: `${stop * 100}%` }} />)}
      </span>
    </div>
    {marker && <span className="ccc-range-marker" style={{ left: position(progress(marker.value, min, max)) }}
      aria-hidden="true" title={marker.label} />}
    <span className="ccc-range-thumb" aria-hidden="true" />
    <input {...props} {...keyboard} ref={input} className="ccc-range-input" type="range"
      min={min} max={max} step={1} value={value} disabled={disabled} aria-disabled={busy && !disabled || undefined} />
  </div>;
}
