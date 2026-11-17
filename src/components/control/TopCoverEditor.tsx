import type { TopCoverValues } from "../../api/client";
import { Surface } from "../Surface";
import { Input } from "../Input";
import { ScaleSlider } from "./ScaleSlider";
import { hexOf, rgbOf } from "./useKeyboardLighting";
import { topCoverText } from "./lighting-option-values";

export function TopCoverEditor({ current, draft, disabled, onChange }: {
  current: TopCoverValues; draft: TopCoverValues; disabled: boolean; onChange: (values: TopCoverValues) => void;
}) {
  return <Surface as="section" variant="inset" className="ccc-lighting-option ccc-top-cover" aria-label="独立顶盖灯">
    <div className="ccc-profile-values"><strong>独立顶盖</strong><p className="ccc-control-caption">当前{topCoverText(current)}</p>
      <svg viewBox="0 0 180 90" role="img" aria-label="独立顶盖颜色预设">
        <rect x="12" y="8" width="156" height="70" rx="8" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
        {[0, 1, 2].map(index => <rect key={index} x={36 + index * 39} y="34" width="30" height="16" rx="3" fill={hexOf(current.color)} opacity={current.brightness > 0 ? 1 : 0.2} />)}
      </svg><span className="ccc-control-caption">颜色示意，非物理位置回读</span>
    </div>
    <div className="ccc-profile-values">
      <ScaleSlider label="顶盖亮度" unit="级" min={0} max={4} current={current.brightness} draft={draft.brightness} disabled={disabled}
        onChange={brightness => onChange({ ...draft, brightness })} />
      <label className="ccc-color-picker">顶盖颜色<Input aria-label="顶盖颜色" type="color" value={hexOf(draft.color)} disabled={disabled}
        onChange={event => onChange({ ...draft, color: rgbOf(event.target.value) })} /></label>
      <span className="ccc-control-caption">亮度0关闭，颜色预设保留。</span>
    </div>
  </Surface>;
}
