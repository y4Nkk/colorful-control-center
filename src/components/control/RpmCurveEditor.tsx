import { Select } from "../Select";
import { useState } from "react";
import type { RpmPoint } from "../../api/client";
import { PointNumber } from "./CurvePointInputs";
import { FanCurveSeries, type FanEditorProps } from "./FanCurveSection";
import { FirmwareCurveHandle } from "./FirmwareCurveHandle";
import { useCurveDraft } from "./useCurveDraft";
import { moveRpmPoint, rpmLimits, rpmStepPath, sameRpmCurve } from "./rpmCurve";

interface Props extends FanEditorProps {
  current: RpmPoint[];
  draft: RpmPoint[];
  maximumRpm: number;
  onChange: (points: RpmPoint[]) => void;
}

export function RpmCurveEditor({ source, current, draft, maximumRpm, editable, busy, onChange, plotRef, ...series }: Props) {
  const [selected, select] = useState(0);
  const edit = useCurveDraft(draft, onChange);
  const limits = rpmLimits(draft, selected), point = draft[selected];
  const changeTemp = (value: number) => (before: RpmPoint[]) => moveRpmPoint(before, selected, value, before[selected].rpmPct);
  const changeRpm = (value: number) => (before: RpmPoint[]) => moveRpmPoint(before, selected, before[selected].tempC, value);
  const differs = !sameRpmCurve(current, draft);
  const fields = <>
    <p className="ccc-control-caption">最大转速 {maximumRpm} RPM</p>
    <div className="ccc-curve-point ccc-rpm-inputs" role="group" aria-label="RPM 曲线数值">
      <label className="ccc-curve-number">级别<Select size="compact" value={String(selected)} label="选择 RPM 级别"
        disabled={!editable} busy={busy}
        options={draft.map((_, index) => ({ value: String(index), label: `第 ${index + 1} 级` }))}
        onChange={value => select(Number(value))} /></label>
      <PointNumber key={`temp${selected}`} label="温度 °C" name="RPM 温度" value={point.tempC} {...limits.temp}
        disabled={!editable} busy={busy} onChange={value => edit.update(changeTemp(value))} onCommit={value => edit.commit(changeTemp(value))} onCancel={edit.cancel} />
      <PointNumber key={`rpm${selected}`} label="目标转速 %" name="RPM 转速" value={point.rpmPct} {...limits.rpm}
        disabled={!editable || selected === draft.length - 1} busy={busy} onChange={value => edit.update(changeRpm(value))}
        onCommit={value => edit.commit(changeRpm(value))} onCancel={edit.cancel} />
    </div>
  </>;
  return <FanCurveSeries {...series} startTemp={0} draftPath={rpmStepPath(draft)}
    currentPath={differs ? rpmStepPath(current) : undefined} fields={fields}>
    {draft.map((p, index) => {
      const bounds = rpmLimits(draft, index);
      return <FirmwareCurveHandle key={index} source={source} plotRef={plotRef} plotMin={series.minTemp} rpmIndex={index}
        selected={selected === index} disabled={!editable} busy={busy} onSelect={() => select(index)}
        x={p.tempC} y={100 - p.rpmPct} temp={p.tempC} tempLimits={bounds.temp} targetLimits={bounds.rpm}
        label={`第 ${index + 1} 级温度与转速`} valueText={`${p.tempC}°C，最大转速的 ${p.rpmPct}%`}
        onMove={(temp, rpm) => edit.update(before => moveRpmPoint(before, index, temp, rpm))}
        onStep={(dt, dr) => edit.update(before => moveRpmPoint(before, index, before[index].tempC + dt, before[index].rpmPct + dr))}
        onCommit={() => edit.commit()} onCancel={edit.cancel} />;
    })}
  </FanCurveSeries>;
}
