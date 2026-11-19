import { Button } from "../Button";
import { Select } from "../Select";
import { useRef, useState, type PointerEvent } from "react";
import type { RpmPoint } from "../../api/client";
import { PointNumber } from "./CurvePointInputs";
import { FanCurveLegend, FanCurvePlot } from "./FanCurvePlot";
import { moveRpmPoint, rpmLimits, rpmStepPath, sameRpmCurve } from "./rpmCurve";

interface Props {
  current: RpmPoint[];
  draft: RpmPoint[];
  maximumRpm: number;
  editable: boolean;
  onChange: (points: RpmPoint[]) => void;
}

export function RpmCurveEditor({ current, draft, maximumRpm, editable, onChange }: Props) {
  const plot = useRef<HTMLDivElement>(null);
  const [selected, select] = useState(0);
  const [dragging, setDragging] = useState<number | null>(null);
  const limits = rpmLimits(draft, selected), point = draft[selected];
  const update = (index: number, temp: number, rpm: number) => onChange(moveRpmPoint(draft, index, temp, rpm));
  const pointer = (event: PointerEvent<HTMLButtonElement>, index: number) => {
    const box = plot.current?.getBoundingClientRect();
    if (box && box.width && box.height) update(index, (event.clientX - box.left) / box.width * 100,
      100 - (event.clientY - box.top) / box.height * 100);
  };
  const differs = !sameRpmCurve(current, draft);
  return <div className="ccc-plot-wrap">
    <FanCurvePlot label="十级温度与目标转速曲线" measure="目标转速" minTemp={0} maxTemp={100}
      plotRef={plot} draftPath={rpmStepPath(draft)} currentPath={differs ? rpmStepPath(current) : undefined}>
      {draft.map((p, index) => <Button variant="handle" type="button" key={index} role="slider" data-rpm-point={index}
        className={`ccc-plot-handle${dragging === index ? " is-dragging" : ""}${selected === index ? " is-selected" : ""}`}
        style={{ left: `${p.tempC}%`, top: `${100 - p.rpmPct}%` }} disabled={!editable}
        aria-label={`第 ${index + 1} 级温度与转速`} aria-valuemin={rpmLimits(draft, index).temp.min}
        aria-valuemax={rpmLimits(draft, index).temp.max} aria-valuenow={p.tempC}
        aria-valuetext={`${p.tempC}°C，最大转速的 ${p.rpmPct}%`} onFocus={() => select(index)}
        onKeyDown={(event) => {
          const step = event.shiftKey ? 5 : 1;
          const delta = ({ ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] } as Record<string, number[]>)[event.key];
          if (delta) { event.preventDefault(); update(index, p.tempC + delta[0], p.rpmPct + delta[1]); }
        }}
        onPointerDown={(event) => { event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); select(index); setDragging(index); pointer(event, index); }}
        onPointerMove={(event) => { if (dragging === index) pointer(event, index); }}
        onPointerUp={() => setDragging(null)} onPointerCancel={() => setDragging(null)} />)}
    </FanCurvePlot>
    <FanCurveLegend hasReference={differs} mainLabel={differs ? "草稿" : "当前设置"}>
      <span>100% = {maximumRpm} RPM</span>
      <span className="ccc-plot-hint">十级阈值 · 新目标 ≥20%</span>
    </FanCurveLegend>
    <div className="ccc-curve-point ccc-rpm-inputs" role="group" aria-label="RPM 曲线数值">
      <label className="ccc-curve-number">级别<Select size="compact" value={String(selected)} label="选择 RPM 级别"
        options={draft.map((_, index) => ({ value: String(index), label: `第 ${index + 1} 级` }))}
        onChange={value => select(Number(value))} /></label>
      <PointNumber key={`temp${selected}`} label="温度 °C" name="RPM 温度" value={point.tempC} {...limits.temp}
        disabled={!editable} onChange={(temp) => update(selected, temp, point.rpmPct)} />
      <PointNumber key={`rpm${selected}`} label="目标转速 %" name="RPM 转速" value={point.rpmPct} {...limits.rpm}
        disabled={!editable || selected === draft.length - 1} onChange={(rpm) => update(selected, point.tempC, rpm)} />
    </div>
  </div>;
}
