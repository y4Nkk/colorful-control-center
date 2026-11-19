/**
 * 大型可编辑风扇曲线图：温度（x）—占空比（y）。
 * T1/D1 与 100°C/100% 为固件锚点；T2/D2、T3/D3 可拖动/键盘调整草稿，
 * 钳位与单调约束来自 curve.ts。当前应用值用安静的参考线显示，
 * 点位标注只在交互时出现，实时温度对应的目标值在图外呈现。
 */
import { Button } from "../Button";
import { useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { CurvePointInputs } from "./CurvePointInputs";
import { FanCurveLegend, FanCurvePlot, fanTemperaturePercent } from "./FanCurvePlot";
import {
  CURVE_TEMP_MAX,
  dutyAtTemp,
  movePoint,
  pointLimits,
  sameDraft,
  type CurveAnchors,
  type CurveDraft,
} from "./curve";
import { fmtInt } from "./format";

const DEFAULT_PLOT_MIN_TEMP = 30;
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

  const plotMinTemp = Math.min(DEFAULT_PLOT_MIN_TEMP, Math.floor(anchors.t1 / 10) * 10);
  const xPct = (temp: number) => fanTemperaturePercent(temp, plotMinTemp, CURVE_TEMP_MAX);
  const linePath = (points: CurveDraft) => `M${anchors.t1},${yPct(anchors.d1Pct)} L${points.t2},${yPct(points.d2Pct)} L${points.t3},${yPct(points.d3Pct)} L${CURVE_TEMP_MAX},0`;
  const differs = !sameDraft(current, draft);
  const projectedDuty = liveTempC == null ? null : dutyAtTemp(liveTempC, anchors, draft);

  const move = (key: "p2" | "p3", temp: number, duty: number) => onChange(movePoint(anchors, draft, key, temp, duty));
  const step = (key: "p2" | "p3", dTemp: number, dDuty: number) => move(key,
    (key === "p2" ? draft.t2 : draft.t3) + dTemp, (key === "p2" ? draft.d2Pct : draft.d3Pct) + dDuty);

  return (
    <div className="ccc-plot-wrap">
      <FanCurvePlot label={title} measure="占空比" minTemp={plotMinTemp} maxTemp={CURVE_TEMP_MAX}
        plotRef={plotRef} draftPath={linePath(draft)} currentPath={differs ? linePath(current) : undefined}>
        <PlotDot x={xPct(anchors.t1)} y={yPct(anchors.d1Pct)} />
        <PlotDot x={100} y={0} />
        <CurveHandle
          point="p2"
          plotRef={plotRef} plotMinTemp={plotMinTemp} disabled={!editable}
          x={xPct(draft.t2)} y={yPct(draft.d2Pct)}
          label="T2/D2 可调点：左右改温度，上下改占空比"
          valueText={`温度 ${draft.t2}°C，占空比 ${draft.d2Pct}%`}
          temp={draft.t2} minTemp={pointLimits(anchors, draft, "p2").temp.min} maxTemp={draft.t3 - 1}
          onValue={(temp, duty) => move("p2", temp, duty)}
          onStep={(dTemp, dDuty) => step("p2", dTemp, dDuty)}
        />
        <CurveHandle
          point="p3"
          plotRef={plotRef} plotMinTemp={plotMinTemp} disabled={!editable}
          x={xPct(draft.t3)} y={yPct(draft.d3Pct)}
          label="T3/D3 可调点：左右改温度，上下改占空比"
          valueText={`温度 ${draft.t3}°C，占空比 ${draft.d3Pct}%`}
          temp={draft.t3} minTemp={draft.t2 + 1} maxTemp={CURVE_TEMP_MAX - 1}
          onValue={(temp, duty) => move("p3", temp, duty)}
          onStep={(dTemp, dDuty) => step("p3", dTemp, dDuty)}
        />
        <PlotLabel point="p2" x={xPct(draft.t2)} y={yPct(draft.d2Pct)} text={`T2 ${draft.t2}°·${draft.d2Pct}%`} />
        <PlotLabel point="p3" x={xPct(draft.t3)} y={yPct(draft.d3Pct)} text={`T3 ${draft.t3}°·${draft.d3Pct}%`} />
      </FanCurvePlot>
      <FanCurveLegend hasReference={differs} mainLabel={differs ? "草稿" : "当前设置"}>
        {liveTempC != null && projectedDuty != null && <span>
          实时 <span className="ccc-mono">{fmtInt(liveTempC)}°C</span> · {differs ? "草稿预估" : "曲线预估"} <span className="ccc-mono">{fmtInt(projectedDuty)}%</span>
        </span>}
        <span className="ccc-plot-hint">{editable ? "拖动圆点 · 方向键微调" : "曲线只读"}</span>
      </FanCurveLegend>
      <CurvePointInputs anchors={anchors} draft={draft} disabled={!editable} onChange={onChange} />
    </div>
  );
}

interface HandleProps {
  point: "p2" | "p3";
  plotRef: RefObject<HTMLDivElement | null>;
  plotMinTemp: number;
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

function CurveHandle({ point, plotRef, plotMinTemp, disabled, x, y, label, valueText, temp, minTemp, maxTemp, onValue, onStep }: HandleProps) {
  const [dragging, setDragging] = useState(false);
  const pointerValues = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const rect = plotRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return null;
    const px = Math.min(100, Math.max(0, ((event.clientX - rect.left) / rect.width) * 100));
    const py = Math.min(100, Math.max(0, ((event.clientY - rect.top) / rect.height) * 100));
    return {
      temp: plotMinTemp + (px / 100) * (CURVE_TEMP_MAX - plotMinTemp),
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
