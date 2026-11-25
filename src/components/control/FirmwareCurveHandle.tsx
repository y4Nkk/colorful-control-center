import { useRef, useState, type PointerEvent, type RefObject } from "react";
import { magnetAt } from "../../lib/range-magnet";
import { Button } from "../Button";
import { CURVE_TEMP_MAX } from "./curve";

const ARROWS: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
const MAGNET_INTERVAL = 10;

interface CurveDrag {
  id: number;
  source: string;
  x: number;
  y: number;
  moved: boolean;
  tempMagnet?: number;
  targetMagnet?: number;
}

interface Props {
  source: string;
  plotRef: RefObject<HTMLDivElement | null>;
  plotMin: number;
  point?: "p2" | "p3";
  rpmIndex?: number;
  x: number;
  y: number;
  temp: number;
  tempLimits: { min: number; max: number };
  targetLimits: { min: number; max: number };
  label: string;
  valueText: string;
  disabled: boolean;
  busy: boolean;
  selected: boolean;
  onSelect: () => void;
  onMove: (temp: number, target: number) => void;
  onStep: (temp: number, target: number) => void;
  onCommit: () => void;
  onCancel: () => void;
}

export function FirmwareCurveHandle({ source, plotRef, plotMin, point, rpmIndex, x, y, temp, tempLimits, targetLimits, label, valueText,
  disabled, busy, selected, onSelect, onMove, onStep, onCommit, onCancel }: Props) {
  const pointer = useRef<CurveDrag | null>(null);
  const keys = useRef<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const locked = disabled || busy;

  const movePointer = (event: PointerEvent<HTMLButtonElement>, gesture: CurveDrag) => {
    const box = plotRef.current?.getBoundingClientRect();
    if (!box?.width || !box.height) return;
    const tempScale = box.width / (CURVE_TEMP_MAX - plotMin);
    const targetScale = box.height / 100;
    const rawTemp = plotMin + (event.clientX - box.left) / tempScale;
    const rawTarget = (box.bottom - event.clientY) / targetScale;
    gesture.tempMagnet = magnetAt(rawTemp, tempScale, gesture.tempMagnet, tempLimits.min, tempLimits.max, MAGNET_INTERVAL);
    gesture.targetMagnet = magnetAt(rawTarget, targetScale, gesture.targetMagnet, targetLimits.min, targetLimits.max, MAGNET_INTERVAL);
    onMove(gesture.tempMagnet ?? rawTemp, gesture.targetMagnet ?? rawTarget);
  };
  const endPointer = (id: number, apply: boolean) => {
    const gesture = pointer.current;
    if (gesture?.id !== id) return;
    pointer.current = null;
    setDragging(false);
    if (apply && gesture.moved && gesture.source === source && !locked) onCommit();
    else if (gesture.moved) onCancel();
  };

  return <Button variant="handle" type="button" role="slider" data-point={point} data-rpm-point={rpmIndex}
    className={`ccc-plot-handle${selected ? " is-selected" : ""}${dragging ? " is-dragging" : ""}`}
    style={{ left: `${x}%`, top: `${y}%` }} disabled={disabled}
    aria-disabled={busy && !disabled || undefined} aria-busy={busy && !disabled || undefined}
    aria-label={label} aria-valuemin={tempLimits.min} aria-valuemax={tempLimits.max} aria-valuenow={temp} aria-valuetext={valueText} aria-orientation="vertical"
    onFocus={onSelect} onBlur={() => { if (keys.current) { keys.current = null; onCancel(); } }}
    onKeyDown={event => {
      const delta = ARROWS[event.key];
      if (!delta) return;
      event.preventDefault();
      if (locked) return;
      keys.current ??= source;
      if (keys.current === source) onStep(delta[0] * (event.shiftKey ? 5 : 1), delta[1] * (event.shiftKey ? 5 : 1));
    }} onKeyUp={event => {
      if (!ARROWS[event.key] || !keys.current) return;
      const started = keys.current;
      keys.current = null;
      if (!locked && started === source) onCommit(); else onCancel();
    }} onPointerDown={event => {
      if (locked || event.button !== 0 || !event.isPrimary) return;
      pointer.current = { id: event.pointerId, source, x: event.clientX, y: event.clientY, moved: false };
      event.currentTarget.setPointerCapture(event.pointerId);
      onSelect();
      setDragging(true);
    }} onPointerMove={event => {
      const gesture = pointer.current;
      if (locked || gesture?.id !== event.pointerId || gesture.source !== source) return;
      if (!gesture.moved && gesture.x === event.clientX && gesture.y === event.clientY) return;
      gesture.moved = true;
      movePointer(event, gesture);
    }} onPointerUp={event => endPointer(event.pointerId, true)}
    onPointerCancel={event => endPointer(event.pointerId, false)} onLostPointerCapture={event => endPointer(event.pointerId, false)} />;
}
