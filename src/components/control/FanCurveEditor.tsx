/**
 * 风扇曲线以温度为横轴，以风扇强度为纵轴。
 * T1/D1 与 100°C/100% 为固件锚点；T2/D2、T3/D3 可拖动/键盘调整草稿，
 * 钳位与单调约束来自 curve.ts。当前应用值用安静的参考线显示，
 * 点位标注只在交互时出现。
 */
import { useState } from "react";
import { CurvePointInputs } from "./CurvePointInputs";
import { FanCurveSeries, type FanEditorProps } from "./FanCurveSection";
import { fanTemperaturePercent } from "./FanCurvePlot";
import { FirmwareCurveHandle } from "./FirmwareCurveHandle";
import { useCurveDraft } from "./useCurveDraft";
import {
  CURVE_TEMP_MAX,
  movePoint,
  pointLimits,
  sameDraft,
  type CurveAnchors,
  type CurveDraft,
} from "./curve";

const yPct = (duty: number) => 100 - duty;

interface FanCurveEditorProps extends FanEditorProps {
  anchors: CurveAnchors;
  /** 已应用的当前曲线，草稿变化时显示参考线 */
  current: CurveDraft;
  draft: CurveDraft;
  onChange: (next: CurveDraft) => void;
}

export function FanCurveEditor({ source, anchors, current, draft, editable, busy, onChange, plotRef, ...series }: FanCurveEditorProps) {
  const [selected, setSelected] = useState<"p2" | "p3" | null>(null);
  const edit = useCurveDraft(draft, onChange);

  const plotMinTemp = series.minTemp;
  const xPct = (temp: number) => fanTemperaturePercent(temp, plotMinTemp, CURVE_TEMP_MAX);
  const linePath = (points: CurveDraft) => `M${anchors.t1},${yPct(anchors.d1Pct)} L${points.t2},${yPct(points.d2Pct)} L${points.t3},${yPct(points.d3Pct)} L${CURVE_TEMP_MAX},0`;
  const differs = !sameDraft(current, draft);
  return <FanCurveSeries {...series} startTemp={anchors.t1} draftPath={linePath(draft)} currentPath={differs ? linePath(current) : undefined}
    fields={<CurvePointInputs anchors={anchors} draft={draft} disabled={!editable} busy={busy} selected={selected} onSelect={setSelected}
      onChange={edit.update} onCommit={edit.commit} onCancel={edit.cancel} />}>
    <PlotDot x={xPct(anchors.t1)} y={yPct(anchors.d1Pct)} />
    <PlotDot x={100} y={0} />
    {(["p2", "p3"] as const).map(point => {
      const second = point === "p2";
      const temp = second ? draft.t2 : draft.t3;
      const duty = second ? draft.d2Pct : draft.d3Pct;
      const limits = pointLimits(anchors, draft, point);
      const name = second ? "T2/D2" : "T3/D3";
      return <FirmwareCurveHandle key={point} source={source} plotRef={plotRef} plotMin={plotMinTemp} point={point}
        disabled={!editable} busy={busy} selected={selected === point} onSelect={() => setSelected(point)}
        x={xPct(temp)} y={yPct(duty)} temp={temp} tempLimits={limits.temp} targetLimits={limits.duty}
        label={`${name} 可调点，左右调温度，上下调风扇强度`} valueText={`温度 ${temp}°C，风扇强度 ${duty}%`}
        onMove={(t, d) => edit.update(before => movePoint(anchors, before, point, t, d))}
        onStep={(dt, dd) => edit.update(before => movePoint(anchors, before, point,
          (second ? before.t2 : before.t3) + dt, (second ? before.d2Pct : before.d3Pct) + dd))}
        onCommit={() => edit.commit()} onCancel={edit.cancel} />;
    })}
    <PlotLabel point="p2" x={xPct(draft.t2)} y={yPct(draft.d2Pct)} text={`T2 ${draft.t2}°·${draft.d2Pct}%`} />
    <PlotLabel point="p3" x={xPct(draft.t3)} y={yPct(draft.d3Pct)} text={`T3 ${draft.t3}°·${draft.d3Pct}%`} />
  </FanCurveSeries>;
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
