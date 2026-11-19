import type { CSSProperties } from "react";
import type { RgbColor } from "../../api/client";
import { KEYBOARD_SCHEMATIC } from "../../lib/perkey-layout";
import { hexOf } from "../../lib/color";
import { KEYBOARD_ZONE_NAMES } from "./useKeyboardLighting";

const { keys, width, height } = KEYBOARD_SCHEMATIC;
const zoneKeys = KEYBOARD_ZONE_NAMES.map((_, index) => keys
  .filter(key => Math.floor((key.x + key.width / 2) / width * KEYBOARD_ZONE_NAMES.length) === index)
  .map(key => <span key={key.id} className="ccc-lighting-board-key"
    style={{ left: `${key.x / width * 100}%`, top: `${key.y / height * 100}%`,
      width: `${key.width / width * 100}%`, height: `${key.height / height * 100}%` }}>{key.legend}</span>));

export function KeyboardLightingPreview({ zones, brightness, maximum, selected, dirty }: {
  zones: ReadonlyArray<{ color: RgbColor | null; enabled: boolean }>;
  brightness: number | null; maximum: number; selected: number | null; dirty: boolean;
}) {
  return <div className="ccc-lighting-board">
    <div className="ccc-lighting-board-head">
      <strong>{dirty ? "草稿预览" : "键盘预览"}</strong>
      <span>{brightness === null ? "保存值未读取 · 键位示意" : brightness === 0 ? "背光关闭 · 键位示意" : "键位示意"}</span>
    </div>
    <div className="ccc-lighting-board-keys" role="img" aria-label="全键盘背光示意"
      style={{ aspectRatio: `${width} / ${height}` }}>
      {zones.map((zone, index) => {
        const color = zone.color && hexOf(zone.color);
        const level = brightness === null || zone.enabled ? brightness : 0;
        return <div key={KEYBOARD_ZONE_NAMES[index]} className="ccc-lighting-board-zone" aria-hidden="true"
          data-keyboard-zone={index} data-zone-color={color ?? "firmware"} data-zone-brightness={level}
          data-selected={selected === index} style={{ "--key-light": color ?? "transparent",
            "--key-intensity": color ? (level ?? 0) / maximum : 0, "--key-zone-start": `${index / zones.length * 100}%` } as CSSProperties}>
          {zoneKeys[index]}
        </div>;
      })}
    </div>
  </div>;
}
