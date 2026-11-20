/**
 * 大型可编辑风扇曲线图：温度（x）—占空比（y）。
 * T1/D1 与 100°C/100% 为固件锚点；T2/D2、T3/D3 可拖动/键盘调整草稿，
 * 钳位与单调约束来自 curve.ts。当前应用值用安静的参考线显示，
 * 点位标注只在交互时出现，实时温度对应的目标值在图外呈现。
 */
import { useRef, useState } from "react";
import { CurvePointInputs } from "./CurvePointInputs";
import { FanEditorLayout, type FanEditorSections } from "./FanEditorLayout";
import { FanCurveLegend, FanCurvePlot, fanTemperaturePercent } from "./FanCurvePlot";
import { FirmwareCurveHandle } from "./FirmwareCurveHandle";
import { useFanEdit } from "./useFanEdit";
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
  sections: FanEditorSections;
  fanName: string;
  source: object;
  anchors: CurveAnchors;
  /** 已应用的当前曲线，草稿变化时显示参考线 */
  current: CurveDraft;
  editable: boolean;
  /** 实时传感器温度，缺失时不显示曲线目标预估 */
  liveTempC: number | null;
  onCommit: (next: CurveDraft) => void;
}

export function FanCurveEditor(props: FanCurveEditorProps) {
  const { sections, fanName, source, anchors, current, editable, liveTempC, onCommit } = props;
  const plotRef = useRef<HTMLDivElement | null>(null);
  const [selected, setSelected] = useState<"p2" | "p3" | null>(null);
  const edit = useFanEdit(source, current, onCommit);
  const draft = edit.value;

  const plotMinTemp = Math.min(DEFAULT_PLOT_MIN_TEMP, Math.floor(anchors.t1 / 10) * 10);
  const xPct = (temp: number) => fanTemperaturePercent(temp, plotMinTemp, CURVE_TEMP_MAX);
  const linePath = (points: CurveDraft) => `M${anchors.t1},${yPct(anchors.d1Pct)} L${points.t2},${yPct(points.d2Pct)} L${points.t3},${yPct(points.d3Pct)} L${CURVE_TEMP_MAX},0`;
  const differs = !sameDraft(current, draft);
  const projectedDuty = liveTempC == null ? null : dutyAtTemp(liveTempC, anchors, draft);

  const plot = <div className="ccc-plot-wrap">
      <FanCurvePlot label={`${fanName} 温度与占空比曲线`} measure="占空比" minTemp={plotMinTemp} maxTemp={CURVE_TEMP_MAX}
        plotRef={plotRef} draftPath={linePath(draft)} currentPath={differs ? linePath(current) : undefined}>
        <PlotDot x={xPct(anchors.t1)} y={yPct(anchors.d1Pct)} />
        <PlotDot x={100} y={0} />
        {(["p2", "p3"] as const).map(point => {
          const second = point === "p2";
          const temp = second ? draft.t2 : draft.t3;
          const duty = second ? draft.d2Pct : draft.d3Pct;
          const limits = pointLimits(anchors, draft, point).temp;
          const name = second ? "T2/D2" : "T3/D3";
          return <FirmwareCurveHandle key={point} source={source} plotRef={plotRef} plotMin={plotMinTemp} point={point}
            disabled={!editable} selected={selected === point} onSelect={() => setSelected(point)}
            x={xPct(temp)} y={yPct(duty)} temp={temp} minTemp={limits.min} maxTemp={limits.max}
            label={`${name} 可调点：左右改温度，上下改占空比`} valueText={`温度 ${temp}°C，占空比 ${duty}%`}
            onMove={(t, d) => edit.update(before => movePoint(anchors, before, point, t, d))}
            onStep={(dt, dd) => edit.update(before => movePoint(anchors, before, point,
              (second ? before.t2 : before.t3) + dt, (second ? before.d2Pct : before.d3Pct) + dd))}
            onCommit={() => edit.commit()} onCancel={edit.cancel} />;
        })}
        <PlotLabel point="p2" x={xPct(draft.t2)} y={yPct(draft.d2Pct)} text={`T2 ${draft.t2}°·${draft.d2Pct}%`} />
        <PlotLabel point="p3" x={xPct(draft.t3)} y={yPct(draft.d3Pct)} text={`T3 ${draft.t3}°·${draft.d3Pct}%`} />
      </FanCurvePlot>
      <FanCurveLegend hasReference={differs} mainLabel={differs ? "草稿" : "当前设置"}>
        {projectedDuty != null && <span>{differs ? "草稿估算" : "曲线估算"} <span className="ccc-mono">{fmtInt(projectedDuty)}%</span></span>}
        <span className="ccc-plot-hint">{editable ? "拖动圆点 · 方向键微调" : "曲线只读"}</span>
      </FanCurveLegend>
    </div>;
  return <FanEditorLayout {...sections} plot={plot} fields={<>
    <h3>{fanName} 曲线点</h3>
    <CurvePointInputs anchors={anchors} draft={draft} disabled={!editable} selected={selected} onSelect={setSelected}
      onChange={edit.update} onCommit={edit.commit} onCancel={edit.cancel} />
  </>} />;
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
