import { useId } from "react";
import type { DisplayMode as DeviceMode } from "../../api/client";
type DisplayMode = Pick<DeviceMode, "width" | "height" | "frequency" | "orientation">;
export const displayDirection = (value: number) => ["横向", "纵向", "横向（翻转）", "纵向（翻转）"][value] ?? "未知方向";

export function DisplayModeVisual({ mode, label, showLabel = true }: { mode: DisplayMode | null; label: string; showLabel?: boolean }) {
  const id = `ccc-display-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const scale = mode && mode.width > 0 && mode.height > 0 ? Math.min(200 / mode.width, 112 / mode.height) : null;
  const width = scale && mode ? mode.width * scale : 170, height = scale && mode ? mode.height * scale : 96;
  const x = (240 - width) / 2, y = 12 + (112 - height) / 2;
  const bezelTop = y - 5;
  const viewportTop = showLabel ? 0 : bezelTop;
  const flipped = mode?.orientation === 2 || mode?.orientation === 3;
  return <div className="ccc-display-mode-visual" role="img" aria-label={`${label}：${mode ? `${displayDirection(mode.orientation)}，${mode.width} × ${mode.height}，${mode.frequency} Hz` : "未读取"}`}>
    {showLabel && <span className="ccc-control-caption">{label}</span>}
    <svg viewBox={`0 ${viewportTop} 240 ${160 - viewportTop}`} aria-hidden="true" data-mode-frequency={mode?.frequency} data-mode-orientation={mode?.orientation}>
      <defs>
        <linearGradient id={`${id}-shell`} x2=".8" y2="1">
          <stop stopColor="var(--ccc-display-art-metal-light)" />
          <stop offset="1" stopColor="var(--ccc-display-art-metal)" />
        </linearGradient>
        <linearGradient id={`${id}-glass`} x1=".08" y1="1" x2=".94" y2="0">
          <stop stopColor="var(--ccc-display-art-depth)" />
          <stop offset=".62" stopColor="var(--ccc-display-art-blue)" />
          <stop offset="1" stopColor="var(--ccc-display-art-violet)" />
        </linearGradient>
        <linearGradient id={`${id}-fold`} x1=".15" y1="1" x2=".9" y2=".05">
          <stop stopColor="var(--ccc-display-art-blue)" />
          <stop offset=".64" stopColor="var(--ccc-display-art-ice)" />
          <stop offset="1" stopColor="var(--ccc-display-art-violet)" />
        </linearGradient>
        <linearGradient id={`${id}-reflection`} x1=".1" y1="0" x2=".64" y2="1">
          <stop stopColor="var(--ccc-hardware-art-highlight)" stopOpacity=".4" />
          <stop offset="1" stopColor="var(--ccc-hardware-art-highlight)" stopOpacity="0" />
        </linearGradient>
        <clipPath id={`${id}-aperture`}><rect x={x} y={y} width={width} height={height} rx="5" /></clipPath>
      </defs>
      <ellipse cx="120" cy="151" rx="39" ry="3.5" fill="var(--ccc-display-art-depth)" opacity=".13" />
      <path d={`M112 ${y + height + 3}h16L130 137Q131 142 136 144h-32q5-2 6-7Z`} fill={`url(#${id}-shell)`} />
      <rect x="89" y="144" width="62" height="5" rx="2.5" fill={`url(#${id}-shell)`} />
      <rect x={x - 5} y={bezelTop} width={width + 10} height={height + 10} rx="9" fill={`url(#${id}-shell)`} />
      <rect x={x - 2} y={y - 2} width={width + 4} height={height + 4} rx="7" fill="var(--ccc-hardware-art-black)" />
      <g clipPath={`url(#${id}-aperture)`}>
        <rect x={x} y={y} width={width} height={height} fill="var(--ccc-display-art-depth)" />
        {mode && <g transform={flipped ? `rotate(180 120 ${y + height / 2})` : undefined}>
          <svg x={x} y={y} width={width} height={height} viewBox="0 0 200 112" preserveAspectRatio="xMidYMid slice">
            <rect width="200" height="112" fill={`url(#${id}-glass)`} />
            <path d="M-14 114C27 101 37 74 62 54c38-31 74 2 105-15 15-8 28-21 41-40v116Z" fill={`url(#${id}-fold)`} />
            <path d="M-12 124c32-1 55-27 82-41 24-12 41-10 69-5 34 6 59-2 74-18v64Z" fill="var(--ccc-display-art-depth)" opacity=".4" />
            <path d="M-6 80C32 82 56 33 96 23c37-10 69 12 108-12V-4H-6Z" fill={`url(#${id}-reflection)`} />
          </svg>
        </g>}
      </g>
    </svg>
    <strong className="ccc-mono">{mode ? `${mode.frequency} Hz` : "—"}</strong>
    <span className="ccc-control-caption">{mode ? `${mode.width} × ${mode.height} · ${displayDirection(mode.orientation)}` : "未读取"}</span>
  </div>;
}
