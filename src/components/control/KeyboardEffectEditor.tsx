import { Select } from "../Select";
import { Button } from "../Button";
import { useState } from "react";
import type { KeyboardEffect, KeyboardZones, RgbColor } from "../../api/client";
import { Checkbox } from "../Checkbox";
import { KeyboardLightingPreview } from "./KeyboardLightingPreview";
import { LightingHueWheel } from "./LightingHueWheel";
import { EFFECTS, KEYBOARD_ZONE_NAMES, usesZoneColors } from "./useKeyboardLighting";

const MAX_BRIGHTNESS = 4;
const BRIGHTNESS_LEVELS = Array.from({ length: MAX_BRIGHTNESS + 1 }, (_, level) => level);
const EFFECT_OPTIONS = EFFECTS.map(item => ({ value: item.id, label: item.label }));

export function KeyboardEffectEditor({ effect, setEffect, allColor, setAllColor, zones, setZones, toggleZone,
  brightness, setBrightness, unavailable, disabled: busy, dirty }: {
  effect: KeyboardEffect | null; setEffect: (effect: KeyboardEffect) => void;
  allColor: RgbColor; setAllColor: (color: RgbColor) => void;
  zones: KeyboardZones; setZones: (zones: KeyboardZones) => void;
  toggleZone: (index: number, enabled: boolean) => void;
  brightness: number; setBrightness: (brightness: number) => void;
  unavailable: boolean; disabled: boolean; dirty: boolean;
}) {
  const [selected, setSelected] = useState(0);
  const disabled = busy || unavailable;
  const zoned = usesZoneColors(effect);
  const editable = zoned || effect === "solid";
  const zoneName = KEYBOARD_ZONE_NAMES[selected];
  const updateZone = (patch: Partial<KeyboardZones[number]>) => {
    const next: KeyboardZones = [...zones];
    next[selected] = { ...next[selected], ...patch };
    setZones(next);
  };
  const preview = zoned ? zones : zones.map(() => ({ enabled: true, color: effect === "solid" ? allColor : null }));
  return <div className="ccc-lighting-editor">
    <div className="ccc-lighting-controls" data-editable-color={editable}>
      <div className="ccc-lighting-mode">
        <span>灯效</span>
        <Select label="灯效" value={effect ?? ""} disabled={disabled} placeholder="选择灯效" options={EFFECT_OPTIONS}
          onChange={value => setEffect(value as KeyboardEffect)} />
        {zoned && <div className="ccc-lighting-zone-controls">
          <div className="ccc-lighting-zones" role="group" aria-label="选择灯区">
            {KEYBOARD_ZONE_NAMES.map((name, index) => <Button key={name} variant="secondary" size="compact"
              aria-label={`编辑${name}`} aria-pressed={selected === index} disabled={disabled} onClick={() => setSelected(index)}>{name}</Button>)}
          </div>
          <Checkbox label={`${zoneName}开关`} checked={zones[selected].enabled} disabled={disabled}
            onChange={event => toggleZone(selected, event.target.checked)} />
        </div>}
        {!editable && effect && <span className="ccc-control-caption">固件配色</span>}
      </div>
      {editable && <LightingHueWheel label={zoned ? `${zoneName}颜色` : "键盘颜色"} color={zoned ? zones[selected].color : allColor}
        disabled={disabled} onChange={zoned ? color => updateZone({ color }) : setAllColor} />}
      <div className="ccc-lighting-brightness">
        <div className="ccc-lighting-brightness-head">
          <span>亮度</span>
          {unavailable && <span className="ccc-control-caption">未上报</span>}
        </div>
        <div className="ccc-lighting-brightness-levels" role="group" aria-label="亮度">
          {BRIGHTNESS_LEVELS.map(level => {
            const selected = !unavailable && level === brightness;
            return <Button key={level} className="ccc-lighting-level" variant={selected ? "primary" : "secondary"} size="compact"
              aria-label={level === 0 ? "关闭" : `${level} 级`} aria-pressed={selected}
              disabled={disabled || !effect} onClick={() => setBrightness(level)}>{level === 0 ? "关闭" : level}</Button>;
          })}
        </div>
      </div>
    </div>
    <KeyboardLightingPreview zones={preview} brightness={unavailable ? null : brightness} maximum={MAX_BRIGHTNESS} selected={zoned ? selected : null} dirty={dirty} />
  </div>;
}
