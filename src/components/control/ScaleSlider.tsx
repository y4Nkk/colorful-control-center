/**
 * 原生数值滑杆（键盘/读屏可用），边界数值在交互时出现。
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
  step?: number;
  /** 已应用的当前值（圆环标记） */
  current: number;
  /** 草稿值（滑块位置） */
  draft: number;
  disabled?: boolean;
  /** 后端未上报该通道：显示"未上报"并禁用 */
  unavailable?: boolean;
  onChange: (value: number) => void;
}

export function ScaleSlider({
  label, unit, min, max, step = 1, current, draft, disabled = false, unavailable = false, onChange,
}: ScaleSliderProps) {
  const pct = (value: number) => Math.max(0, Math.min(100, ((value - min) / Math.max(1e-6, max - min)) * 100));
  const dirty = !unavailable && draft !== current;
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
      <div className="ccc-slider-body">
        {dirty && (
          <span
            className="ccc-slider-current"
            style={{ left: `${pct(current)}%` }}
            title={`当前 ${fmtInt(current)} ${unit}`}
            aria-hidden="true"
          />
        )}
        <Slider
          min={min}
          max={max}
          step={step}
          value={unavailable ? min : draft}
          disabled={disabled || unavailable}
          aria-label={label}
          aria-valuetext={unavailable ? "未上报" : `${fmtInt(draft)} ${unit}`}
          onChange={(event) => onChange(Number(event.target.value))}
        />
      </div>
      <div className="ccc-slider-scale ccc-mono" aria-hidden="true">
        <span>{fmtInt(min)}</span>
        <span>{fmtInt(max)}</span>
      </div>
    </div>
  );
}
