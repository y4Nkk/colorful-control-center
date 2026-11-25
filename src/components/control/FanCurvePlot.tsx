import type { ReactNode, Ref } from "react";

const TEMPERATURE_TICKS = [20, 40, 60, 80];
const PERCENT_TICKS = [100, 80, 60, 40, 20, 0];

export function fanTemperaturePercent(temp: number, minTemp: number, maxTemp: number): number {
  return (temp - minTemp) / (maxTemp - minTemp) * 100;
}
interface PlotProps {
  label: string;
  measure: "占空比" | "目标转速";
  minTemp: number;
  maxTemp: number;
  startTemp: number;
  draftPath: string;
  currentPath?: string;
  plotRef: Ref<HTMLDivElement>;
  children: ReactNode;
}

export function FanCurvePlot({ label, measure, minTemp, maxTemp, startTemp, draftPath, currentPath, plotRef, children }: PlotProps) {
  const temperatures = [minTemp, ...TEMPERATURE_TICKS.filter(temp => temp > minTemp && temp < maxTemp), maxTemp];
  return <div className="ccc-plot-frame">
    <span className="ccc-plot-measure" aria-hidden="true">{measure}</span>
    <div className="ccc-plot-ylabels ccc-mono" aria-hidden="true">
      {PERCENT_TICKS.map(value => <span key={value} style={{ top: `${100 - value}%` }}>{value}%</span>)}
    </div>
    <div className="ccc-plot" ref={plotRef} role="group" aria-label={label}
      aria-description={currentPath ? "实线为曲线草稿，虚线为当前设置。" : undefined}>
      <svg className="ccc-plot-svg" viewBox={`${minTemp} 0 ${maxTemp - minTemp} 100`} preserveAspectRatio="none" aria-hidden="true">
        {draftPath && <path className="ccc-plot-area" d={`${draftPath} L${maxTemp},100 H${startTemp} Z`} />}
        <g className="ccc-plot-grid">
          {temperatures.map(temp => <line key={temp} x1={temp} x2={temp} y1={0} y2={100} vectorEffect="non-scaling-stroke" />)}
          {PERCENT_TICKS.map(value => <line key={value} x1={minTemp} x2={maxTemp} y1={100 - value} y2={100 - value} vectorEffect="non-scaling-stroke" />)}
        </g>
        <path className="ccc-plot-origin" d={`M${minTemp},0 V100 H${maxTemp}`} vectorEffect="non-scaling-stroke" />
        {currentPath && <path d={currentPath} className="ccc-plot-current" vectorEffect="non-scaling-stroke" />}
        <path d={draftPath} className="ccc-plot-draft" vectorEffect="non-scaling-stroke" />
      </svg>
      {children}
    </div>
    <div className="ccc-plot-axis ccc-mono" aria-hidden="true">
      {temperatures.map(temp => <span key={temp} style={{ left: `${fanTemperaturePercent(temp, minTemp, maxTemp)}%` }}>{temp}</span>)}
    </div>
    <span className="ccc-plot-unit" aria-hidden="true">温度 °C</span>
  </div>;
}
