import { Input } from "../Input";
import { Tooltip } from "../Tooltip";
import { useEffect, useRef, useState } from "react";
import { clampInt, movePoint, pointLimits, type CurveAnchors, type CurveDraft } from "./curve";

type PointEdit = (draft: CurveDraft) => CurveDraft;

export function CurvePointInputs({ anchors, draft, disabled, busy, selected, onSelect, onChange, onCommit, onCancel }: {
  anchors: CurveAnchors; draft: CurveDraft; disabled: boolean; busy: boolean; selected: "p2" | "p3" | null;
  onSelect: (point: "p2" | "p3") => void; onChange: (edit: PointEdit) => void;
  onCommit: (edit: PointEdit) => void; onCancel: () => void;
}) {
  return <div className="ccc-curve-values" role="group" aria-label="曲线数值">
    <div className="ccc-curve-columns" aria-hidden="true"><span /><span>温度 °C</span><span>风扇强度 %</span></div>
    {(["p2", "p3"] as const).map((point) => {
      const label = point === "p2" ? "T2" : "T3";
      const temp = point === "p2" ? draft.t2 : draft.t3;
      const duty = point === "p2" ? draft.d2Pct : draft.d3Pct;
      const limits = pointLimits(anchors, draft, point);
      const changeTemp = (next: number): PointEdit => before => movePoint(anchors, before, point, next, point === "p2" ? before.d2Pct : before.d3Pct);
      const changeDuty = (next: number): PointEdit => before => movePoint(anchors, before, point, point === "p2" ? before.t2 : before.t3, next);
      return <div className="ccc-curve-point" key={point} data-selected={selected === point || undefined} onFocus={() => onSelect(point)}>
        <strong className="ccc-mono">{label}</strong>
        <PointNumber name={`${label} 温度`} value={temp} {...limits.temp} disabled={disabled} busy={busy}
          onChange={next => onChange(changeTemp(next))} onCommit={next => onCommit(changeTemp(next))} onCancel={onCancel} />
        <PointNumber name={`${label} 风扇强度`} value={duty} {...limits.duty} disabled={disabled} busy={busy}
          onChange={next => onChange(changeDuty(next))} onCommit={next => onCommit(changeDuty(next))} onCancel={onCancel} />
      </div>;
    })}
  </div>;
}

export function PointNumber({ label, name, value, min, max, disabled, busy, onChange, onCommit, onCancel }: {
  label?: string; name: string; value: number; min: number; max: number; disabled: boolean; busy: boolean;
  onChange: (next: number) => void; onCommit: (next: number) => void; onCancel: () => void;
}) {
  const [text, setText] = useState(String(value));
  const edited = useRef(false);
  useEffect(() => { setText(String(value)); }, [value, min, max]);
  const invalid = text !== String(value) && (text.trim() === "" || !Number.isFinite(Number(text)) || Number(text) < min || Number(text) > max);
  const cancel = () => { edited.current = false; setText(String(value)); onCancel(); };
  const commit = () => {
    if (disabled || busy || !edited.current) return;
    if (!text.trim() || !Number.isFinite(Number(text))) { cancel(); return; }
    const next = clampInt(Number(text), min, max);
    edited.current = false;
    setText(String(value));
    onCommit(next);
  };
  return <label className="ccc-curve-number">
    {label && <span>{label}</span>}
    <Tooltip content={`${min}–${max}`}>
      <Input size="compact" type="number" className="ccc-mono" aria-label={name} aria-invalid={invalid}
        min={min} max={max} step={1} value={text} disabled={disabled}
        readOnly={busy} aria-disabled={busy && !disabled || undefined} aria-busy={busy && !disabled || undefined}
        onChange={(event) => {
          if (busy) return;
          edited.current = true;
          const next = event.target.value;
          setText(next);
          const numeric = Number(next);
          if (next.trim() && Number.isFinite(numeric) && numeric >= min && numeric <= max)
            onChange(Math.round(numeric));
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") { event.preventDefault(); commit(); }
          if (event.key === "Escape") { event.preventDefault(); cancel(); }
        }} />
    </Tooltip>
  </label>;
}
