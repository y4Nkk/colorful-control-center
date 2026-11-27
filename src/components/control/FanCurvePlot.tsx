import type { ReactNode, Ref } from "react";
import { CURVE_TEMP_MAX } from "./curve";

const TEMPERATURE_TICKS = [20, 40, 60, 80];
const PERCENT_TICKS = [100, 80, 60, 40, 20, 0];

export function fanTemperaturePercent(temp: number, minTemp: number, maxTemp: number): number {
  return (temp - minTemp) / (maxTemp - minTemp) * 100;
}
interface PlotProps {
  label: string;
  measure: "风扇强度" | "目标转速";
  minTemp: number;
  plotRef: Ref<HTMLDivElement>;
  children: ReactNode;
}

export function FanCurvePlot({ label, measure, minTemp, plotRef, children }: PlotProps) {
  const maxTemp = CURVE_TEMP_MAX;
  const temperatures = [minTemp, ...TEMPERATURE_TICKS.filter(temp => temp > minTemp && temp < maxTemp), maxTemp];
  return <div className="ccc-plot-frame">
    <div className="ccc-plot-ylabels ccc-mono" aria-hidden="true">
      {PERCENT_TICKS.map(value => <span key={value} style={{ top: `${100 - value}%` }}>{value}%</span>)}
    </div>
    <div className="ccc-plot" ref={plotRef} role="group" aria-label={label}
      aria-description="点选曲线或风扇名称进入点位调节。编辑时用淡色虚线标记当前设置。">
      <svg className="ccc-plot-svg" viewBox={`${minTemp} 0 ${maxTemp - minTemp} 100`} preserveAspectRatio="none" aria-hidden="true">
        <g className="ccc-plot-grid">
          {temperatures.map(temp => <line key={temp} x1={temp} x2={temp} y1={0} y2={100} vectorEffect="non-scaling-stroke" />)}
          {PERCENT_TICKS.map(value => <line key={value} x1={minTemp} x2={maxTemp} y1={100 - value} y2={100 - value} vectorEffect="non-scaling-stroke" />)}
        </g>
        <path className="ccc-plot-origin" d={`M${minTemp},0 V100 H${maxTemp}`} vectorEffect="non-scaling-stroke" />
      </svg>
      {children}
    </div>
    <div className="ccc-plot-axis ccc-mono" aria-hidden="true">
      {temperatures.map(temp => <span key={temp} style={{ left: `${fanTemperaturePercent(temp, minTemp, maxTemp)}%` }}>{temp}</span>)}
    </div>
    <div className="ccc-plot-units" aria-hidden="true">
      <span className="ccc-plot-measure">{measure}</span>
      <span className="ccc-plot-unit">温度 °C</span>
    </div>
  </div>;
}

export function FanCurveLine({ minTemp, startTemp, draftPath, currentPath, fill, onChoose }: {
  minTemp: number; startTemp: number; draftPath: string; currentPath?: string; fill: boolean;
  onChoose: () => void;
}) {
  return <svg className="ccc-plot-svg" viewBox={`${minTemp} 0 ${CURVE_TEMP_MAX - minTemp} 100`} preserveAspectRatio="none" aria-hidden="true">
    {fill && draftPath && <path className="ccc-plot-area" d={`${draftPath} L${CURVE_TEMP_MAX},100 H${startTemp} Z`} />}
    {currentPath && <path d={currentPath} className="ccc-plot-current" vectorEffect="non-scaling-stroke" />}
    <path d={draftPath} className="ccc-plot-draft" vectorEffect="non-scaling-stroke" />
    <path d={draftPath} className="ccc-plot-hit" vectorEffect="non-scaling-stroke" onClick={onChoose} />
  </svg>;
}
