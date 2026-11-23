import { Select } from "../Select";
import { useRef, useState } from "react";
import type { RpmPoint } from "../../api/client";
import { PointNumber } from "./CurvePointInputs";
import { FanCurveLegend, FanCurvePlot } from "./FanCurvePlot";
import { FanCurveSection } from "./FanCurveSection";
import { FirmwareCurveHandle } from "./FirmwareCurveHandle";
import { useFanEdit } from "./useFanEdit";
import { moveRpmPoint, rpmLimits, rpmStepPath, sameRpmCurve } from "./rpmCurve";

interface Props {
  status?: string;
  fanName: string;
  source: object;
  current: RpmPoint[];
  maximumRpm: number;
  editable: boolean;
  onCommit: (points: RpmPoint[]) => void;
}

export function RpmCurveEditor({ status, fanName, source, current, maximumRpm, editable, onCommit }: Props) {
  const plot = useRef<HTMLDivElement>(null);
  const [selected, select] = useState(0);
  const edit = useFanEdit(source, current, onCommit);
  const draft = edit.value;
  const limits = rpmLimits(draft, selected), point = draft[selected];
  const changeTemp = (value: number) => (before: RpmPoint[]) => moveRpmPoint(before, selected, value, before[selected].rpmPct);
  const changeRpm = (value: number) => (before: RpmPoint[]) => moveRpmPoint(before, selected, before[selected].tempC, value);
  const differs = !sameRpmCurve(current, draft);
  const chart = <div className="ccc-plot-wrap">
    <FanCurvePlot label="十级温度与目标转速曲线" measure="目标转速" minTemp={0} maxTemp={100} startTemp={0}
      plotRef={plot} draftPath={rpmStepPath(draft)} currentPath={differs ? rpmStepPath(current) : undefined}>
      {draft.map((p, index) => {
        const bounds = rpmLimits(draft, index).temp;
        return <FirmwareCurveHandle key={index} source={source} plotRef={plot} plotMin={0} rpmIndex={index}
          selected={selected === index} disabled={!editable} onSelect={() => select(index)}
          x={p.tempC} y={100 - p.rpmPct} temp={p.tempC} minTemp={bounds.min} maxTemp={bounds.max}
          label={`第 ${index + 1} 级温度与转速`} valueText={`${p.tempC}°C，最大转速的 ${p.rpmPct}%`}
          onMove={(temp, rpm) => edit.update(before => moveRpmPoint(before, index, temp, rpm))}
          onStep={(dt, dr) => edit.update(before => moveRpmPoint(before, index, before[index].tempC + dt, before[index].rpmPct + dr))}
          onCommit={() => edit.commit()} onCancel={edit.cancel} />;
      })}
    </FanCurvePlot>
    <FanCurveLegend hasReference={differs} mainLabel={differs ? "草稿" : "当前设置"}>
      <span>100% = {maximumRpm} RPM</span>
      <span className="ccc-plot-hint">十级阈值 · 新目标 ≥20%</span>
    </FanCurveLegend>
  </div>;
  return <FanCurveSection status={status} plot={chart} fields={<>
    <h4>{fanName} 曲线点</h4>
    <div className="ccc-curve-point ccc-rpm-inputs" role="group" aria-label="RPM 曲线数值">
      <label className="ccc-curve-number">级别<Select size="compact" value={String(selected)} label="选择 RPM 级别"
        disabled={!editable}
        options={draft.map((_, index) => ({ value: String(index), label: `第 ${index + 1} 级` }))}
        onChange={value => select(Number(value))} /></label>
      <PointNumber key={`temp${selected}`} label="温度 °C" name="RPM 温度" value={point.tempC} {...limits.temp}
        disabled={!editable} onChange={value => edit.update(changeTemp(value))} onCommit={value => edit.commit(changeTemp(value))} onCancel={edit.cancel} />
      <PointNumber key={`rpm${selected}`} label="目标转速 %" name="RPM 转速" value={point.rpmPct} {...limits.rpm}
        disabled={!editable || selected === draft.length - 1} onChange={value => edit.update(changeRpm(value))}
        onCommit={value => edit.commit(changeRpm(value))} onCancel={edit.cancel} />
    </div>
  </>} />;
}
