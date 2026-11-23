/**
 * 数值滑杆的业务读数和边界。
 * 应用中的当前值以圆环标记；草稿即滑块本身。
 * 当前值与草稿一致时不显示标记。
 */
import { Slider } from "../Slider";
import { fmtInt } from "./format";

interface ScaleSliderProps {
  label: string;
  unit: string;
  min: number;
  max: number;
  magnetInterval?: number;
  /** 已应用的当前值（圆环标记） */
  current: number;
  /** 草稿值（滑块位置） */
  draft: number;
  disabled?: boolean;
  /** 后端未上报该通道：显示"未上报"并禁用 */
  unavailable?: boolean;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  onCancel?: () => void;
}

export function ScaleSlider({
  label, unit, min, max, magnetInterval, current, draft, disabled = false, unavailable = false, onChange, onCommit, onCancel,
}: ScaleSliderProps) {
  const dirty = !onCommit && !unavailable && draft !== current;
  return (
    <div className={`ccc-slider${disabled ? " is-disabled" : ""}`}>
      <div className="ccc-slider-head">
        <span className="ccc-slider-label">{label}</span>
        <span className="ccc-mono ccc-slider-read">
          {unavailable ? (
            "未上报"
          ) : dirty ? (
            <>
              <span className="ccc-slider-old">{fmtInt(current)}</span>
              {" → "}
              <span>{fmtInt(draft)}</span> {unit}
            </>
          ) : (
            <>
              {fmtInt(draft)} {unit}
            </>
          )}
        </span>
      </div>
      <Slider
        min={min}
        max={max}
        magnetInterval={magnetInterval}
        value={unavailable ? min : draft}
        marker={dirty ? { value: current, label: `当前 ${fmtInt(current)} ${unit}` } : undefined}
        disabled={disabled || unavailable}
        aria-label={label}
        aria-valuetext={unavailable ? "未上报" : `${fmtInt(draft)} ${unit}`}
        onChange={onChange}
        onCommit={onCommit}
        onCancel={onCancel}
      />
      <div className="ccc-slider-scale ccc-mono" aria-hidden="true">
        <span>{fmtInt(min)}</span>
        <span>{fmtInt(max)}</span>
      </div>
    </div>
  );
}
