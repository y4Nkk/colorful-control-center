import { useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { FAN_LABELS, type FanId } from "../../api/client";
import { FanCurveLine, FanCurvePlot } from "./FanCurvePlot";

export interface FanCurveSelection {
  frontFan: FanId | null;
  fieldId: (id: FanId) => string;
  chooseFan: (id: FanId) => void;
}

interface CurveLayout {
  plotRef: RefObject<HTMLDivElement | null>;
  fieldsHost: HTMLDivElement | null;
  minTemp: number;
  fill: boolean;
  selection: FanCurveSelection;
  onActivate: (id: FanId) => void;
}

export interface FanEditorProps extends CurveLayout {
  id: FanId;
  source: string;
  editable: boolean;
  busy: boolean;
}

export function FanCurveSection({ rpm, minTemp, fanCount, selection, onActivate, children }: {
  rpm: boolean;
  minTemp: number;
  fanCount: number;
  selection: FanCurveSelection;
  onActivate: (id: FanId) => void;
  children: (layout: CurveLayout) => ReactNode;
}) {
  const plotRef = useRef<HTMLDivElement>(null);
  const [fieldsHost, setFieldsHost] = useState<HTMLDivElement | null>(null);
  return <div className="ccc-fan-curve">
    <div className="ccc-fan-curve-body">
      <FanCurvePlot label={rpm ? "十级温度与目标转速曲线" : "温度与风扇强度曲线"}
        measure={rpm ? "目标转速" : "风扇强度"} minTemp={minTemp} plotRef={plotRef}>
        {children({ plotRef, fieldsHost, minTemp, fill: fanCount === 1, selection, onActivate })}
      </FanCurvePlot>
      <div className="ccc-fan-fields" ref={setFieldsHost} />
    </div>
  </div>;
}

export function FanCurveSeries({ id, minTemp, fill, fieldsHost, selection, onActivate, startTemp, draftPath, currentPath, fields, children }:
  Omit<CurveLayout, "plotRef"> & { id: FanId; startTemp: number; draftPath: string; currentPath?: string; fields: ReactNode; children: ReactNode }) {
  const name = FAN_LABELS[id];
  const selected = selection.frontFan === id;
  const activate = () => onActivate(id);
  return <>
    <div className="ccc-curve-series" data-fan={id} data-front={selected || undefined}
      role="group" aria-label={`${name} 风扇曲线`} onFocus={activate} onPointerDown={activate}>
      <FanCurveLine minTemp={minTemp} startTemp={startTemp} fill={fill} draftPath={draftPath} currentPath={currentPath}
        onChoose={() => selection.chooseFan(id)} />
      {children}
    </div>
    {fieldsHost && createPortal(
      <div id={selection.fieldId(id)} className="ccc-fan-field-group" data-fan={id} data-selected={selected || undefined}
        role="group" aria-label={`${name} 曲线点`} tabIndex={-1} onFocus={activate}>
        <h5 className="ccc-fan-name">{name} 曲线点</h5>
        {fields}
      </div>, fieldsHost,
    )}
  </>;
}
