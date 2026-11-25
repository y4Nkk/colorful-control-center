import { useRef, useState, type ChangeEvent, type KeyboardEvent, type PointerEvent } from "react";
import { magnetAt } from "../lib/range-magnet";

interface SliderInput {
  min: number;
  max: number;
  magnetInterval?: number;
  disabled: boolean;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  onCancel?: () => void;
}

interface Drag {
  pointerId: number;
  grabOffset: number;
  start: number;
  value: number;
  lastX?: number;
  magnet?: number;
}

const RANGE_KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);

function sampleDrag(drag: Drag, clientX: number, bounds: DOMRect, { min, max, magnetInterval }: SliderInput) {
  const pixelsPerUnit = bounds.width / (max - min);
  const raw = min + (clientX - drag.grabOffset - bounds.left) / pixelsPerUnit;
  drag.magnet = magnetInterval ? magnetAt(raw, pixelsPerUnit, drag.magnet, min, max, magnetInterval) : undefined;
  return drag.magnet ?? Math.max(min, Math.min(max, Math.round(raw)));
}

function startDrag(event: PointerEvent, field: HTMLInputElement, bounds: DOMRect, min: number, max: number): Drag {
  const start = field.valueAsNumber;
  const thumbX = bounds.left + (start - min) / (max - min) * bounds.width;
  const delta = event.clientX - thumbX;
  const grabbed = Math.abs(delta) <= (field.getBoundingClientRect().width - bounds.width) / 2;
  return { pointerId: event.pointerId, grabOffset: grabbed ? delta : 0, start, value: start,
    lastX: grabbed ? event.clientX : undefined };
}

export function useSliderInput(options: SliderInput) {
  const { min, max, disabled, onChange, onCommit, onCancel } = options;
  const input = useRef<HTMLInputElement>(null);
  const rail = useRef<HTMLSpanElement>(null);
  const gesture = useRef<Drag | "keyboard" | null>(null);
  const [dragging, setDragging] = useState(false);

  function cancel() {
    const active = gesture.current;
    if (!active) return;
    gesture.current = null;
    if (active !== "keyboard") setDragging(false);
    onCancel?.();
  }

  function update(drag: Drag, clientX: number) {
    if (drag.lastX === clientX) return;
    drag.lastX = clientX;
    const bounds = rail.current?.getBoundingClientRect();
    if (!bounds?.width || max <= min) return;
    const next = sampleDrag(drag, clientX, bounds, options);
    if (next === drag.value) return;
    drag.value = next;
    onChange(next);
  }

  function begin(event: PointerEvent<HTMLDivElement>) {
    const field = input.current;
    const bounds = rail.current?.getBoundingClientRect();
    if (disabled || !event.isPrimary || event.button !== 0 || gesture.current || !field || !bounds?.width || max <= min) return;
    event.preventDefault();
    field.focus({ preventScroll: true });
    const drag = startDrag(event, field, bounds, min, max);
    gesture.current = drag;
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
    update(drag, event.clientX);
  }

  function handlePointer(event: PointerEvent<HTMLDivElement>) {
    const drag = gesture.current;
    if (!drag || drag === "keyboard" || drag.pointerId !== event.pointerId) return;
    if (disabled || event.type === "pointercancel" || event.type === "lostpointercapture") return cancel();
    update(drag, event.clientX);
    if (event.type !== "pointerup") return;
    gesture.current = null;
    setDragging(false);
    if (drag.value !== drag.start) onCommit?.(drag.value);
    else onCancel?.();
  }

  return {
    input, rail, dragging,
    pointer: { onPointerDown: begin, onPointerMove: handlePointer, onPointerUp: handlePointer,
      onPointerCancel: handlePointer, onLostPointerCapture: handlePointer },
    keyboard: {
      onChange: (event: ChangeEvent<HTMLInputElement>) => {
        if (disabled) return;
        (gesture.current === null ? onCommit ?? onChange : onChange)(event.currentTarget.valueAsNumber);
      },
      onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => {
        if (event.key === "Escape" && gesture.current) cancel();
        if (!RANGE_KEYS.has(event.key)) return;
        if (disabled || (gesture.current && gesture.current !== "keyboard")) event.preventDefault();
        else if (onCommit) gesture.current = "keyboard";
      },
      onKeyUp: (event: KeyboardEvent<HTMLInputElement>) => {
        if (gesture.current !== "keyboard" || !RANGE_KEYS.has(event.key)) return;
        if (disabled) cancel();
        else {
          gesture.current = null;
          onCommit?.(event.currentTarget.valueAsNumber);
        }
      },
      onBlur: cancel,
    },
  };
}
