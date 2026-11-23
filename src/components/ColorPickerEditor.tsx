import { useState, type CSSProperties, type PointerEvent } from "react";
import type { RgbColor } from "../api/client";
import { hexOf, hsvOf, parseHex, rgbOfHsv, sameColor, type HsvColor } from "../lib/color";
import { Input } from "./Input";
import { Slider } from "./Slider";

const CHANNELS = ["r", "g", "b"] as const;
const unit = (value: number) => Math.min(1, Math.max(0, value));

export function ColorPickerEditor({ value, onChange }: { value: RgbColor; onChange: (color: RgbColor) => void }) {
  const [position, setPosition] = useState<{ hsv: HsvColor; color: RgbColor } | null>(null);
  const [entry, setEntry] = useState<{ field: keyof RgbColor | "hex"; text: string; color: RgbColor; invalid: boolean } | null>(null);
  const hsv = position && sameColor(position.color, value) ? position.hsv : hsvOf(value, position?.hsv.h);
  const editing = entry && sameColor(entry.color, value) ? entry : null;
  const update = (color: RgbColor) => { if (!sameColor(color, value)) onChange(color); };
  const edit = (field: keyof RgbColor | "hex", text: string, color: RgbColor | null) => {
    setEntry({ field, text, color: color ?? value, invalid: color === null });
    if (color) update(color);
  };
  const move = (next: HsvColor) => {
    const color = rgbOfHsv(next);
    // Retain the selected hue for gray and the pointer position between RGB rounding steps.
    setPosition({ hsv: next, color });
    update(color);
  };
  const pick = (event: PointerEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    move({ ...hsv, s: unit((event.clientX - bounds.left) / bounds.width), v: 1 - unit((event.clientY - bounds.top) / bounds.height) });
  };
  return <div className="ccc-color-picker-editor">
    <div className="ccc-color-picker-plane" aria-hidden="true"
      style={{ "--picker-hue": hsv.h, "--picker-saturation": `${hsv.s * 100}%`, "--picker-darkness": `${(1 - hsv.v) * 100}%` } as CSSProperties}
      onPointerDown={event => {
        if (event.button !== 0 || !event.isPrimary) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        pick(event);
      }}
      onPointerMove={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) pick(event); }}>
      <span className="ccc-color-picker-marker" />
    </div>
    <Slider appearance="hue" aria-label="色相" min={0} max={359} value={Math.round(hsv.h) % 360}
      onChange={h => move({ ...hsv, h })} />
    <div className="ccc-color-picker-fields">
      {CHANNELS.map(channel => {
        const current = editing?.field === channel ? editing : null;
        return <label key={channel}><span>{channel.toUpperCase()}</span>
          <Input className="ccc-mono" size="compact" type="number" inputMode="numeric" min={0} max={255} step={1}
            aria-invalid={current?.invalid} value={current?.text ?? value[channel]} onBlur={() => setEntry(null)}
            onChange={event => {
              const { value: text, valueAsNumber: number } = event.currentTarget;
              edit(channel, text, Number.isInteger(number) && number >= 0 && number <= 255 ? { ...value, [channel]: number } : null);
            }} />
        </label>;
      })}
      <label><span>HEX</span>
        <Input className="ccc-mono" size="compact" spellCheck={false} autoComplete="off"
          aria-invalid={editing?.field === "hex" && editing.invalid}
          value={editing?.field === "hex" ? editing.text : hexOf(value).toUpperCase()}
          onBlur={() => setEntry(null)} onChange={event => edit("hex", event.currentTarget.value, parseHex(event.currentTarget.value))} />
      </label>
    </div>
  </div>;
}
