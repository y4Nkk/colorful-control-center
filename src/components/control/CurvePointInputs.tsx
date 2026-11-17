import { Input } from "../Input";
import { useEffect, useState } from "react";
import { clampInt, movePoint, pointLimits, type CurveAnchors, type CurveDraft } from "./curve";

export function CurvePointInputs({ anchors, draft, disabled, onChange }: {
  anchors: CurveAnchors; draft: CurveDraft; disabled: boolean; onChange: (next: CurveDraft) => void;
}) {
  return <div className="ccc-curve-values" role="group" aria-label="曲线数值">
    {(["p2", "p3"] as const).map((point) => {
      const label = point === "p2" ? "T2" : "T3";
      const temp = point === "p2" ? draft.t2 : draft.t3;
      const duty = point === "p2" ? draft.d2Pct : draft.d3Pct;
      const limits = pointLimits(anchors, draft, point);
      return <div className="ccc-curve-point" key={point}>
        <strong className="ccc-mono">{label}</strong>
        <PointNumber label="温度 °C" name={`${label} 温度`} value={temp} {...limits.temp} disabled={disabled}
          onChange={(next) => onChange(movePoint(anchors, draft, point, next, duty))} />
        <PointNumber label="占空比 %" name={`${label} 占空比`} value={duty} {...limits.duty} disabled={disabled}
          onChange={(next) => onChange(movePoint(anchors, draft, point, temp, next))} />
      </div>;
    })}
  </div>;
}

export function PointNumber({ label, name, value, min, max, disabled, onChange }: {
  label: string; name: string; value: number; min: number; max: number; disabled: boolean;
  onChange: (next: number) => void;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => { setText(String(value)); }, [value, min, max]);
  const invalid = text !== String(value) && (text.trim() === "" || !Number.isFinite(Number(text)) || Number(text) < min || Number(text) > max);
  const commit = () => {
    if (text === String(value)) return;
    const next = text.trim() === "" || !Number.isFinite(Number(text)) ? value : clampInt(Number(text), min, max);
    setText(String(next));
    if (next !== value) onChange(next);
  };
  return <label className="ccc-curve-number">
    <span>{label}</span>
    <Input size="compact" type="number" className="ccc-mono" aria-label={name} aria-invalid={invalid}
      title={`${min}–${max}`} min={min} max={max} step={1} value={text} disabled={disabled}
      onChange={(event) => {
        const next = event.target.value;
        setText(next);
        const numeric = Number(next);
        if (next.trim() && Number.isFinite(numeric) && numeric >= min && numeric <= max)
          onChange(Math.round(numeric));
      }}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") { event.preventDefault(); commit(); }
        if (event.key === "Escape") { event.preventDefault(); setText(String(value)); }
      }} />
  </label>;
}
