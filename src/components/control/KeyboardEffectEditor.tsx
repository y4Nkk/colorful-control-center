import { Select } from "../Select";
import { Button } from "../Button";
import { useState } from "react";
import type { KeyboardEffect, KeyboardZones, RgbColor } from "../../api/client";
import { Checkbox } from "../Checkbox";
import { Tabs } from "../Tabs";
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
  const solid = effect === "solid";
  const updateZone = (index: number, color: RgbColor) => {
    const next: KeyboardZones = [...zones];
    next[index] = { ...next[index], color };
    setZones(next);
  };
  const preview = zoned ? zones : zones.map(() => ({ enabled: true, color: solid ? allColor : null }));
  return <div className="ccc-lighting-editor">
    <div className="ccc-lighting-controls" data-editable-color={solid}>
      <div className="ccc-lighting-mode">
        <span>灯效</span>
        <Select label="灯效" value={effect ?? ""} disabled={disabled} placeholder="选择灯效" options={EFFECT_OPTIONS}
          onChange={value => setEffect(value as KeyboardEffect)} />
        {!zoned && !solid && effect && <span className="ccc-control-caption">固件配色</span>}
      </div>
      {solid && <LightingHueWheel label="键盘颜色" color={allColor} disabled={disabled} onChange={setAllColor} />}
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
    {zoned && <div className="ccc-lighting-zone-editor">
      <Tabs label="编辑灯区" value={selected} onChange={setSelected} disabled={disabled} tabs={KEYBOARD_ZONE_NAMES.map((name, index) => ({
        value: index, label: name, content: <div className="ccc-lighting-zone-controls">
          <Checkbox label={`${name}开关`} checked={zones[index].enabled} disabled={disabled}
            onChange={event => toggleZone(index, event.target.checked)} />
          <LightingHueWheel label={`${name}颜色`} color={zones[index].color} disabled={disabled}
            onChange={color => updateZone(index, color)} />
        </div>,
      }))} />
    </div>}
    <KeyboardLightingPreview zones={preview} brightness={unavailable ? null : brightness} maximum={MAX_BRIGHTNESS} selected={zoned ? selected : null} dirty={dirty} />
  </div>;
}
