import { useId, type CSSProperties } from "react";
import type { RgbColor } from "../../api/client";
import { KEYBOARD_SCHEMATIC } from "../../lib/perkey-layout";
import { hexOf } from "../../lib/color";
import { KEYBOARD_ZONE_NAMES } from "./useKeyboardLighting";

const { keys, width, height } = KEYBOARD_SCHEMATIC;
const zoneWidth = width / KEYBOARD_ZONE_NAMES.length;
const zoneKeys = KEYBOARD_ZONE_NAMES.map((_, index) => keys
  .filter(key => Math.floor((key.x + key.width / 2) / zoneWidth) === index)
  .map(key => <g key={key.id} className="ccc-lighting-board-key">
    <rect className="ccc-lighting-key-face" x={key.x} y={key.y} width={key.width} height={key.height} rx={width * .0065} />
    <text className="ccc-lighting-key-label" x={key.x + key.width / 2} y={key.y + key.height / 2}
      fontSize={width * .0125} textAnchor="middle" dominantBaseline="central">{key.legend}</text>
  </g>));

export function KeyboardLightingPreview({ zones, brightness, maximum, selected, dirty }: {
  zones: ReadonlyArray<{ color: RgbColor | null; enabled: boolean }>;
  brightness: number | null; maximum: number; selected: number | null; dirty: boolean;
}) {
  const id = useId();
  const status = brightness === null ? "保存值未读取" : brightness === 0 ? "背光关闭" : null;
  return <div className="ccc-lighting-board">
    <div className="ccc-lighting-board-head">
      <h3>{dirty ? "草稿预览" : "键盘预览"}</h3>
      {status && <span>{status}</span>}
    </div>
    <div className="ccc-lighting-board-deck">
      <svg className="ccc-lighting-board-keys" role="img" aria-label="全键盘背光示意" viewBox={`0 0 ${width} ${height}`}>
        {zones.map((zone, index) => {
          const color = zone.color && hexOf(zone.color);
          const level = brightness === null || zone.enabled ? brightness : 0;
          const intensity = color ? (level ?? 0) / maximum : 0;
          const glow = `${id}-${index}`;
          return <g key={KEYBOARD_ZONE_NAMES[index]} className="ccc-lighting-board-zone" aria-hidden="true"
            data-keyboard-zone={index} data-zone-color={color ?? "firmware"} data-zone-brightness={level}
            data-selected={selected === index} style={{ "--key-light": color ?? "transparent",
              "--key-intensity": intensity } as CSSProperties}>
            {color && intensity > 0 && <>
              <defs><radialGradient id={glow}>
                <stop stopColor={color} stopOpacity={intensity * .36} /><stop offset="1" stopColor={color} stopOpacity="0" />
              </radialGradient></defs>
              <rect x={index * zoneWidth} width={zoneWidth} height={height} fill={`url(#${glow})`} />
            </>}
            {zoneKeys[index]}
          </g>;
        })}
      </svg>
    </div>
  </div>;
}
