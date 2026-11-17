import { Select } from "../Select";
import { Button } from "../Button";
import { Input } from "../Input";
import { useState } from "react";
import type { KeyboardEffect, KeyboardZones, RgbColor } from "../../api/client";
import { Checkbox } from "../Checkbox";
import { EFFECTS, hexOf, rgbOf } from "./useKeyboardLighting";

const ZONE_NAMES = ["左区", "中区", "右区"];
export function KeyboardEffectEditor({ effect, setEffect, allColor, setAllColor, zones, setZones, brightness, disabled }: {
  effect: KeyboardEffect | null; setEffect: (effect: KeyboardEffect) => void;
  allColor: RgbColor; setAllColor: (color: RgbColor) => void;
  zones: KeyboardZones; setZones: (zones: KeyboardZones) => void; brightness: number; disabled: boolean;
}) {
  const [selected, setSelected] = useState(0);
  const zoned = effect === "zones" || effect === "breath";
  const editable = zoned || effect === "solid";
  const updateZone = (patch: Partial<KeyboardZones[number]>) => setZones(zones.map((zone, index) => index === selected ? { ...zone, ...patch } : zone) as KeyboardZones);
  return <div className="ccc-keyboard-editor">
    <label className="ccc-lighting-mode">灯效
      <Select label="灯效" value={effect ?? ""} disabled={disabled} placeholder="选择灯效"
        options={EFFECTS.map(item => ({ value: item.id, label: item.label }))}
        onChange={value => setEffect(value as KeyboardEffect)} />
    </label>
    <div className="ccc-keyboard-zones" aria-label="键盘灯光草稿预览">
      {zones.map((zone, index) => <Button variant="selection" type="button" key={index} className="ccc-keyboard-zone" aria-label={`编辑${ZONE_NAMES[index]}`}
        aria-pressed={zoned ? selected === index : undefined} disabled={disabled || !zoned} onClick={() => setSelected(index)}>
        <svg className="ccc-keyboard-zone-art" viewBox="0 0 124 72" aria-hidden="true" data-keyboard-zone={index}
          data-zone-color={editable ? hexOf(zoned ? zone.color : allColor) : "firmware"} data-zone-brightness={brightness}
          opacity={brightness === 0 || zoned && !zone.enabled ? .3 : 1}>
          <rect x="1" y="1" width="122" height="70" rx="8" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
          {Array.from({ length: 16 }, (_, key) => <g key={key}>
            <rect x={7 + key % 4 * 28} y={6 + Math.floor(key / 4) * 16} width="25" height="13" rx="3" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" strokeWidth=".6" />
            <rect x={9 + key % 4 * 28} y={15 + Math.floor(key / 4) * 16} width="21" height="2" rx="1"
              fill={editable ? hexOf(zoned ? zone.color : allColor) : "var(--ccc-text-muted)"} opacity={brightness === 0 ? .2 : .5 + brightness * .125} />
          </g>)}
        </svg><span>{ZONE_NAMES[index]}</span>
      </Button>)}
    </div>
    {editable ? <div className="ccc-lighting-color-row">
      <label className="ccc-color-picker">{zoned ? `${ZONE_NAMES[selected]}颜色` : "键盘颜色"}
        <Input type="color" aria-label={zoned ? `${ZONE_NAMES[selected]}颜色` : "键盘颜色"} value={hexOf(zoned ? zones[selected].color : allColor)}
          disabled={disabled} onChange={(e) => { const color = rgbOf(e.target.value); if (zoned) updateZone({ color }); else setAllColor(color); }} />
      </label>
      {zoned && <Checkbox label={`${ZONE_NAMES[selected]}开关`} checked={zones[selected].enabled} disabled={disabled}
        onChange={(event) => updateZone({ enabled: event.target.checked })} />}
    </div> : effect && <span className="ccc-chip">固件配色</span>}
  </div>;
}
