import { useMemo, useState } from "react";
import { Button } from "../Button";
import { Input } from "../Input";
import { ScrollArea } from "../ScrollArea";
import { Surface } from "../Surface";
import { perkeyLayout } from "../../lib/perkey-layout";
import type { PerkeyDevice, RgbColor } from "../../api/client";
import { hexOf, rgbOf } from "./useKeyboardLighting";

export function PerkeyKeycaps({ colors, device, disabled, onChange }: {
  colors: RgbColor[]; device: PerkeyDevice; disabled: boolean; onChange: (colors: RgbColor[]) => void;
}) {
  const layout = useMemo(() => perkeyLayout(device), [device]);
  const [selected, setSelected] = useState<number[]>([]), [paint, setPaint] = useState("#0a6cf5");
  const [matrix, setMatrix] = useState(false);
  const select = (ids: number[], multiple: boolean) => setSelected(previous => !multiple ? ids :
    ids.every(id => previous.includes(id)) ? previous.filter(id => !ids.includes(id)) : [...new Set([...previous, ...ids])]);
  const keycaps = !!layout && !matrix;
  return <div className="ccc-perkey-static">
    {!layout && <p className="ccc-control-caption">键帽布局尚未识别，显示灯位矩阵；不将灯位编号当作物理按键。</p>}
    {keycaps ? <ScrollArea surface="inset" aria-label="键帽颜色草稿">
      <div className="ccc-perkey-board" style={{ aspectRatio: `${layout.width} / ${layout.height}` }}>
        {layout.keys.map(key => <Button variant="keycap" key={key.id} className="ccc-perkey-cap" disabled={disabled}
          aria-label={`${key.label} · 灯位 ${key.leds.map(id => `${Math.floor(id / 20) + 1}-${id % 20 + 1}`).join("、")}`}
          aria-pressed={key.leds.every(id => selected.includes(id))} title={`${key.label} · ${key.leds.length} 个灯位`}
          style={{ left: `${key.x / layout.width * 100}%`, top: `${key.y / layout.height * 100}%`, width: `${key.width / layout.width * 100}%`, height: `${key.height / layout.height * 100}%` }}
          onClick={event => select(key.leds, event.ctrlKey || event.metaKey)}>
          <span className="ccc-perkey-legend">{key.legend}</span>
          <span className="ccc-perkey-colors" aria-hidden="true">{key.leds.map(id => <span key={id} style={{ background: hexOf(colors[id]) }} />)}</span>
        </Button>)}
      </div>
    </ScrollArea> : <Surface as="div" variant="inset" className="ccc-perkey-grid" aria-label="灯位矩阵草稿">
      {colors.map((color, i) => <Button variant="swatch" key={i} className="ccc-perkey-key" aria-label={`灯位 ${Math.floor(i / 20) + 1}-${i % 20 + 1}`}
        aria-pressed={selected.includes(i)} disabled={disabled} style={{ background: hexOf(color) }}
        onClick={event => select([i], event.ctrlKey || event.metaKey)} />)}
    </Surface>}
    <div className="ccc-actions">
      <Button variant="ghost" size="compact" disabled={disabled} onClick={() => setSelected(colors.map((_, i) => i))}>全选灯位</Button>
      <Button icon="delete" variant="ghost" size="compact" disabled={disabled} onClick={() => setSelected([])}>清除选择</Button>
      {layout && <Button variant="ghost" size="compact" disabled={disabled} onClick={() => setMatrix(value => !value)}>{matrix ? "显示键帽" : "显示灯位矩阵"}</Button>}
      <label className="ccc-color-picker">所选颜色<Input type="color" aria-label="逐键颜色" value={paint} disabled={disabled} onChange={e => setPaint(e.target.value)} /></label>
      <Button variant="secondary" size="compact" disabled={disabled || selected.length === 0} onClick={() => onChange(colors.map((color, i) => selected.includes(i) ? rgbOf(paint) : color))}>填充所选键</Button>
    </div>
    <p className="ccc-control-caption">已选 {selected.length} 个灯位 · Ctrl / ⌘ 多选</p>
  </div>;
}
