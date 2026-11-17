/**
 * 大型可编辑风扇曲线图：温度（x）—占空比（y）。
 * T1/D1 与 100°C/100% 为固件锚点；T2/D2、T3/D3 可拖动/键盘调整草稿，
 * 钳位与单调约束来自 curve.ts。当前应用值用安静的参考线显示，
 * 点位标注只在交互时出现，实时温度对应的目标值在图外呈现。
 */
import { Button } from "../Button";
import { useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { CurvePointInputs } from "./CurvePointInputs";
import {
  CURVE_TEMP_MAX,
  CURVE_TEMP_MIN,
  dutyAtTemp,
  movePoint,
  pointLimits,
  type CurveAnchors,
  type CurveDraft,
} from "./curve";
import { fmtInt } from "./format";

const TEMP_AXIS = [40, 60, 80, 100] as const;
const DUTY_LABELS = [100, 60, 20] as const;

const xPct = (temp: number) => ((temp - CURVE_TEMP_MIN) / (CURVE_TEMP_MAX - CURVE_TEMP_MIN)) * 100;
const yPct = (duty: number) => 100 - duty;

interface FanCurveEditorProps {
  title: string;
  anchors: CurveAnchors;
  /** 已应用的当前曲线，草稿变化时显示参考线 */
  current: CurveDraft;
  /** 编辑草稿（实线与手柄显示） */
  draft: CurveDraft;
  editable: boolean;
  /** 实时传感器温度，缺失时不显示曲线目标预估 */
  liveTempC: number | null;
  onChange: (next: CurveDraft) => void;
}

export function FanCurveEditor(props: FanCurveEditorProps) {
  const { title, anchors, current, draft, editable, liveTempC, onChange } = props;
  const plotRef = useRef<HTMLDivElement | null>(null);

  const linePath = (points: CurveDraft, strokeClass: string) => {
    const d = `M${xPct(anchors.t1)},${yPct(anchors.d1Pct)} L${xPct(points.t2)},${yPct(points.d2Pct)} L${xPct(points.t3)},${yPct(points.d3Pct)} L100,0`;
    return <path d={d} className={strokeClass} vectorEffect="non-scaling-stroke" />;
  };
  const differs = draft.t2 !== current.t2 || draft.d2Pct !== current.d2Pct
    || draft.t3 !== current.t3 || draft.d3Pct !== current.d3Pct;
  const projTemp = liveTempC == null ? null : Math.min(CURVE_TEMP_MAX, Math.max(CURVE_TEMP_MIN, liveTempC));
  const projDuty = projTemp == null ? null : dutyAtTemp(projTemp, anchors, draft);

  const move = (key: "p2" | "p3", temp: number, duty: number) => onChange(movePoint(anchors, draft, key, temp, duty));
  const step = (key: "p2" | "p3", dTemp: number, dDuty: number) => move(key,
    (key === "p2" ? draft.t2 : draft.t3) + dTemp, (key === "p2" ? draft.d2Pct : draft.d3Pct) + dDuty);

  return (
    <div className="ccc-plot-wrap">
      <div className="ccc-plot-ylabels" aria-hidden="true">
        {DUTY_LABELS.map((duty) => (
          <span key={duty} className="ccc-mono" style={{ top: `${yPct(duty)}%` }}>{duty}%</span>
        ))}
      </div>
      <div className="ccc-plot" ref={plotRef} role="group" aria-label={title}>
        <svg className="ccc-plot-svg" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          {differs && linePath(current, "ccc-plot-current")}
          {linePath(draft, "ccc-plot-draft")}
        </svg>
        <PlotDot x={xPct(anchors.t1)} y={yPct(anchors.d1Pct)} />
         <PlotDot x={100} y={0} />
        <CurveHandle
          point="p2"
          plotRef={plotRef} disabled={!editable}
          x={xPct(draft.t2)} y={yPct(draft.d2Pct)}
          label="T2/D2 可调点：左右改温度，上下改占空比"
          valueText={`温度 ${draft.t2}°C，占空比 ${draft.d2Pct}%`}
          temp={draft.t2} minTemp={pointLimits(anchors, draft, "p2").temp.min} maxTemp={draft.t3 - 1}
          onValue={(temp, duty) => move("p2", temp, duty)}
          onStep={(dTemp, dDuty) => step("p2", dTemp, dDuty)}
        />
        <CurveHandle
          point="p3"
          plotRef={plotRef} disabled={!editable}
          x={xPct(draft.t3)} y={yPct(draft.d3Pct)}
          label="T3/D3 可调点：左右改温度，上下改占空比"
          valueText={`温度 ${draft.t3}°C，占空比 ${draft.d3Pct}%`}
          temp={draft.t3} minTemp={draft.t2 + 1} maxTemp={CURVE_TEMP_MAX - 1}
          onValue={(temp, duty) => move("p3", temp, duty)}
          onStep={(dTemp, dDuty) => step("p3", dTemp, dDuty)}
        />
        <PlotLabel point="p2" x={xPct(draft.t2)} y={yPct(draft.d2Pct)} text={`T2 ${draft.t2}°·${draft.d2Pct}%`} />
        <PlotLabel point="p3" x={xPct(draft.t3)} y={yPct(draft.d3Pct)} text={`T3 ${draft.t3}°·${draft.d3Pct}%`} />
      </div>
      <div aria-hidden="true" />
      <div className="ccc-plot-axis">
        {TEMP_AXIS.map((temp) => (
          <span key={temp} className="ccc-mono" style={{ left: `${xPct(temp)}%` }}>{temp}°</span>
        ))}
      </div>
      <CurveLegend differs={differs} editable={editable} />
      {projTemp != null && projDuty != null && <span className="ccc-plot-preview ccc-mono">
        实时 {fmtInt(liveTempC!)}°C · {differs ? "草稿预估" : "曲线目标"} {fmtInt(projDuty)}%
      </span>}
      <CurvePointInputs anchors={anchors} draft={draft} disabled={!editable} onChange={onChange} />
     </div>
   );
 }

function CurveLegend({ differs, editable }: { differs: boolean; editable: boolean }) {
  return <div className="ccc-plot-key" aria-hidden="true">
    <span className={`ccc-key-swatch ${differs ? "is-current" : "is-draft"}`} /><span>当前设置</span>
    {differs && <><span className="ccc-key-swatch is-draft" /><span>草稿</span></>}
    <span className="ccc-plot-hint">{editable ? "拖动圆点 · 方向键微调" : "曲线只读"}</span>
  </div>;
}

interface HandleProps {
  point: "p2" | "p3";
  plotRef: RefObject<HTMLDivElement | null>;
  disabled: boolean;
  x: number;
  y: number;
  label: string;
  valueText: string;
  temp: number;
  minTemp: number;
  maxTemp: number;
  onValue: (temp: number, duty: number) => void;
  onStep: (dTemp: number, dDuty: number) => void;
}

function CurveHandle({ point, plotRef, disabled, x, y, label, valueText, temp, minTemp, maxTemp, onValue, onStep }: HandleProps) {
  const [dragging, setDragging] = useState(false);
  const pointerValues = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const rect = plotRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return null;
    const px = Math.min(100, Math.max(0, ((event.clientX - rect.left) / rect.width) * 100));
    const py = Math.min(100, Math.max(0, ((event.clientY - rect.top) / rect.height) * 100));
    return {
      temp: CURVE_TEMP_MIN + (px / 100) * (CURVE_TEMP_MAX - CURVE_TEMP_MIN),
      duty: 100 - py,
    };
  };
  const onKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    const stepSize = event.shiftKey ? 5 : 1;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-stepSize, 0],
      ArrowRight: [stepSize, 0],
      ArrowUp: [0, stepSize],
      ArrowDown: [0, -stepSize],
    };
    const move = moves[event.key];
    if (!move) return;
    event.preventDefault();
    onStep(move[0], move[1]);
  };
   if (disabled) return <PlotDot x={x} y={y} />;
  return (
    <Button variant="handle"
      type="button"
      data-point={point}
      role="slider"
      className={`ccc-plot-handle${dragging ? " is-dragging" : ""}`}
      style={{ left: `${x}%`, top: `${y}%` }}
      aria-label={label}
      aria-valuemin={minTemp}
      aria-valuemax={maxTemp}
      aria-valuenow={temp}
      aria-valuetext={valueText}
      aria-orientation="vertical"
      onKeyDown={onKeyDown}
      onPointerDown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
        const values = pointerValues(event);
        if (values) onValue(values.temp, values.duty);
      }}
      onPointerMove={(event) => {
        if (!dragging) return;
        const values = pointerValues(event);
        if (values) onValue(values.temp, values.duty);
      }}
      onPointerUp={() => setDragging(false)}
      onPointerCancel={() => setDragging(false)}
    />
  );
}

 function PlotDot({ x, y }: { x: number; y: number }) {
  return <span className="ccc-plot-dot" style={{ left: `${x}%`, top: `${y}%` }} aria-hidden="true" />;
}

function PlotLabel({ x, y, text, point }: { x: number; y: number; text: string; point: "p2" | "p3" }) {
  return (
    <span
      className="ccc-plot-label"
      data-point={point}
      style={{ left: `${x}%`, top: `${y}%`, transform: `translate(${-x}%, ${y < 15 ? "var(--ccc-space-2)" : "calc(-100% - var(--ccc-space-2))"})` }}
      aria-hidden="true"
    >
      {text}
    </span>
  );
}
