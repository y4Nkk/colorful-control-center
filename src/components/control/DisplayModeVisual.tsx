import type { DisplayMode as DeviceMode } from "../../api/client";
type DisplayMode = Pick<DeviceMode, "width" | "height" | "frequency" | "orientation">;
export const displayDirection = (value: number) => ["横向", "纵向", "横向（翻转）", "纵向（翻转）"][value] ?? "未知方向";

export function DisplayModeVisual({ mode, label }: { mode: DisplayMode | null; label: string }) {
  const scale = mode && mode.width > 0 && mode.height > 0 ? Math.min(200 / mode.width, 112 / mode.height) : null;
  const width = scale && mode ? mode.width * scale : 170, height = scale && mode ? mode.height * scale : 96;
  const x = (240 - width) / 2, y = 12 + (112 - height) / 2;
  return <div className="ccc-display-mode-visual" role="img" aria-label={`${label}：${mode ? `${displayDirection(mode.orientation)}，${mode.width} × ${mode.height}，${mode.frequency} Hz` : "未读取"}`}>
    <span className="ccc-control-caption">{label}</span>
    <svg viewBox="0 0 240 160" aria-hidden="true" data-mode-frequency={mode?.frequency} data-mode-orientation={mode?.orientation}>
      <rect x={x - 5} y={y - 5} width={width + 10} height={height + 10} rx="9" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
      <rect x={x} y={y} width={width} height={height} rx="5" fill="var(--ccc-content)" />
      <g transform={mode && (mode.orientation === 2 || mode.orientation === 3) ? `rotate(180 120 ${y + height / 2})` : undefined}>
      <rect x={x + width * .08} y={y + height * .14} width={width * .28} height={height * .68} rx="3" fill="currentColor" opacity={mode ? .14 : .04} />
      <rect x={x + width * .41} y={y + height * .14} width={width * .51} height={height * .68} rx="3" fill="currentColor" opacity={mode ? .06 : .02} />
      </g>
      <path d={`M120 ${y + height + 5}V143m-24 3h48`} fill="none" stroke="var(--ccc-text-muted)" strokeWidth="3" strokeLinecap="round" />
    </svg>
    <strong className="ccc-mono">{mode ? `${mode.frequency} Hz` : "—"}</strong>
    <span className="ccc-control-caption">{mode ? `${mode.width} × ${mode.height} · ${displayDirection(mode.orientation)}` : "未读取"}</span>
  </div>;
}
