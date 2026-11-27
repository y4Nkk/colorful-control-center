import { useId } from "react";

type GraphicsRoute = "integrated" | "discrete" | "hybrid" | "dynamic";
type NodeProps = { connected: boolean; selected: boolean; surface: string };

function IntegratedChip({ connected, selected, surface }: NodeProps) {
  return <g opacity={connected ? 1 : .34}>
    <path d="M13.5 5V2.5M18.5 5V2.5M23.5 5V2.5M13.5 27v2.5M18.5 27v2.5M23.5 27v2.5M7.5 11H5m2.5 5H5m2.5 5H5m24.5-10H32m-2.5 5H32m-2.5 5H32" strokeWidth="1.8" />
    <rect x="7.5" y="5" width="22" height="22" rx="5" fill={connected ? surface : "var(--ccc-content-muted)"}
      stroke="var(--graphic-edge)" strokeWidth="1.25" data-solid={connected && selected || undefined} />
    <rect x="12.5" y="10" width="12" height="12" rx="3" fill="var(--graphic-core)" stroke="none" />
    <rect x="16" y="13.5" width="5" height="5" rx="1" fill="var(--graphic-detail)" stroke="none" />
  </g>;
}

function DiscreteCard({ connected, selected, surface }: NodeProps) {
  return <g opacity={connected ? 1 : .34}>
    <path d="M12 65.5h12.5l-2 4h-9Z" fill="currentColor" opacity=".6" stroke="none" />
    <path d="M4 47.5v21" strokeWidth="1.8" />
    <rect x="6" y="49" width="25.5" height="17.5" rx="4" fill={connected ? surface : "var(--ccc-content-muted)"}
      stroke="var(--graphic-edge)" strokeWidth="1.25" data-solid={connected && selected || undefined} />
    <g fill="var(--graphic-core)" stroke="none">
      <circle cx="13.5" cy="57.75" r="4.35" /><circle cx="24" cy="57.75" r="4.35" />
    </g>
    <g fill="var(--graphic-detail)" stroke="none">
      <circle cx="13.5" cy="57.75" r="1.4" /><circle cx="24" cy="57.75" r="1.4" />
    </g>
  </g>;
}

export function GraphicsModeArtwork({ route, selected }: { route: GraphicsRoute; selected: boolean }) {
  const id = `ccc-graphics-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const surface = `url(#${id}-surface)`;
  const integrated = route !== "discrete";
  const discrete = route !== "integrated";
  const dynamic = route === "dynamic";
  const junction = dynamic ? 74.5 : 104.5;

  return <svg className="ccc-graphics-choice-art" viewBox="0 0 148 74" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false"
    data-selected={selected || undefined}>
    <defs>
      <linearGradient id={`${id}-surface`} x2=".9" y2="1">
        <stop stopColor="var(--graphic-surface-light)" />
        <stop offset="1" stopColor="var(--graphic-surface-depth)" />
      </linearGradient>
      <linearGradient id={`${id}-screen`} x1="0" y1="1" x2="1" y2="0">
        <stop stopColor="var(--graphic-core)" />
        <stop offset="1" stopColor="var(--graphic-screen-light)" />
      </linearGradient>
    </defs>
    <g data-routes strokeDasharray={dynamic ? ".1 5.5" : undefined}>
      {integrated && <path d={`M31.5 16H53a10.5 10.5 0 0 1 10.5 10.5a10.5 10.5 0 0 0 10.5 10.5H${junction}`} />}
      {route === "hybrid"
        ? <path d="M18.5 46.5V34.5" />
        : discrete && <path d={`M31 58H53a10.5 10.5 0 0 0 10.5-10.5a10.5 10.5 0 0 1 10.5-10.5H${junction}`} />}
      {dynamic && <path d="M94.5 37h10" />}
    </g>
    <path d="M104 33.5 109 37l-5 3.5Z" fill="currentColor" stroke="none" />
    {route === "hybrid" && <path d="M15 37.5 18.5 32.5l3.5 5Z" fill="currentColor" stroke="none" />}
    <IntegratedChip connected={integrated} selected={selected} surface={surface} />
    <DiscreteCard connected={discrete} selected={selected} surface={surface} />
    <path d="M126 47h6l1 6.5h3v2.5h-14v-2.5h3Z" fill="var(--graphic-edge)" stroke="none" />
    <rect x="112" y="25.5" width="34" height="23" rx="5" fill={surface} stroke="var(--graphic-edge)" strokeWidth="1.25" />
    <rect x="115.5" y="29" width="27" height="15.5" rx="2.5" fill={`url(#${id}-screen)`} stroke="none" />
    <path d="M118 29h22.5L130 37.5h-14.5V31.5Q115.5 29 118 29Z" fill="var(--graphic-detail)" opacity=".24" stroke="none" />
    {dynamic && <g data-switch strokeWidth="1.65">
      <circle cx="84.5" cy="37" r="10" fill={surface} stroke="var(--graphic-edge)" />
      <path d="M79.75 34h9.5m-2.75-2.75 2.75 2.75-2.75 2.75M89.25 40h-9.5m2.75-2.75-2.75 2.75 2.75 2.75" />
    </g>}
  </svg>;
}
