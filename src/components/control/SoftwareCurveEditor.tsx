import { Button } from "../Button";
import { Select } from "../Select";
import { useRef, useState, type PointerEvent } from "react";
import type { SoftwarePoint } from "../../api/client";
import { PointNumber } from "./CurvePointInputs";
import { addSoftwarePoint, moveSoftwarePoint, softwareLimits, softwarePath } from "./softwareCurve";

export function SoftwareCurveEditor({ current, points, disabled, onChange }: {
  current: SoftwarePoint[] | null; points: SoftwarePoint[]; disabled: boolean; onChange: (points: SoftwarePoint[]) => void;
}) {
  const plot = useRef<HTMLDivElement>(null);
  const [selectedIndex, select] = useState(0), [dragging, setDragging] = useState<number | null>(null);
  const index = Math.min(selectedIndex, points.length - 1), point = points[index];
  const limits = softwareLimits(points, index), fixed = index === points.length - 1;
  const move = (i: number, temp: number, duty: number) => onChange(moveSoftwarePoint(points, i, temp, duty));
  const pointer = (event: PointerEvent<HTMLButtonElement>, i: number) => {
    const box = plot.current?.getBoundingClientRect();
    if (box?.width && box.height) move(i, 20 + (event.clientX - box.left) / box.width * 75, 100 - (event.clientY - box.top) / box.height * 100);
  };
  return <div className="ccc-plot-wrap ccc-point-editor" aria-label="软件自由曲线编辑器">
    <div className="ccc-plot-ylabels" aria-hidden="true">{[100, 60, 20].map((v) => <span key={v} style={{ top: `${100 - v}%` }}>{v}%</span>)}</div>
    <div className="ccc-plot" ref={plot} role="group" aria-label="软件温度与占空比曲线">
      <svg className="ccc-plot-svg" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        {current && JSON.stringify(current) !== JSON.stringify(points) && <path d={softwarePath(current)} className="ccc-plot-current" vectorEffect="non-scaling-stroke" />}
        <path d={softwarePath(points)} className="ccc-plot-draft" vectorEffect="non-scaling-stroke" />
      </svg>
      {points.map((p, i) => <Button variant="handle" type="button" key={i} role="slider" data-software-point={i}
        className={`ccc-plot-handle${index === i ? " is-selected" : ""}${dragging === i ? " is-dragging" : ""}`}
        style={{ left: `${(p.tempC - 20) / 75 * 100}%`, top: `${100 - p.dutyPct}%` }} disabled={disabled}
        aria-label={`软件曲线第 ${i + 1} 点${i === points.length - 1 ? "（固定安全端点）" : ""}`}
        aria-valuemin={softwareLimits(points, i).temp.min} aria-valuemax={softwareLimits(points, i).temp.max}
        aria-valuenow={p.tempC} aria-valuetext={`${p.tempC}°C，占空比 ${p.dutyPct}%`} onFocus={() => select(i)}
        onKeyDown={(event) => {
          const step = event.shiftKey ? 5 : 1;
          const d = ({ ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] } as Record<string, number[]>)[event.key];
          if (d) { event.preventDefault(); move(i, p.tempC + d[0], p.dutyPct + d[1]); }
        }}
        onPointerDown={(event) => { event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); select(i); setDragging(i); pointer(event, i); }}
        onPointerMove={(event) => { if (dragging === i) pointer(event, i); }} onPointerUp={() => setDragging(null)} onPointerCancel={() => setDragging(null)} />)}
    </div>
    <span aria-hidden="true" /><div className="ccc-plot-axis" aria-hidden="true">{[20, 40, 60, 80, 95].map((v) => <span key={v} style={{ left: `${(v - 20) / 75 * 100}%` }}>{v}°</span>)}</div>
    <div className="ccc-plot-key"><span>{point.tempC}°C · {point.dutyPct}%</span><span className="ccc-plot-hint">拖动圆点 · 方向键微调 · 95°C 熔断</span></div>
    <div className="ccc-curve-point ccc-rpm-inputs" role="group" aria-label="软件曲线数值">
      <label className="ccc-curve-number">控制点<Select size="compact" label="选择软件控制点" value={String(index)} disabled={disabled}
        options={points.map((_, i) => ({ value: String(i), label: `第 ${i + 1} 点` }))}
        onChange={value => select(Number(value))} /></label>
      <PointNumber label="温度 °C" name="软件点温度" value={point.tempC} {...limits.temp} disabled={disabled || fixed} onChange={(v) => move(index, v, point.dutyPct)} />
      <PointNumber label="占空比 %" name="软件点占空比" value={point.dutyPct} {...limits.duty} disabled={disabled || fixed} onChange={(v) => move(index, point.tempC, v)} />
    </div>
    <div className="ccc-actions ccc-software-point-actions">
      <Button icon="add" variant="ghost" size="compact" type="button" disabled={disabled || points.length >= 12} onClick={() => onChange(addSoftwarePoint(points))}>添加点</Button>
      <Button icon="delete" variant="ghost" size="compact" type="button" disabled={disabled || fixed || points.length <= 2}
        onClick={() => { onChange(points.filter((_, i) => i !== index)); select(Math.max(0, index - 1)); }}>删除点</Button>
      <span className="ccc-plot-hint">{points.length} / 12 点</span>
    </div>
  </div>;
}
