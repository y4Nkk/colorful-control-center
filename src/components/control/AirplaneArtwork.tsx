import { useId } from "react";

export function AirplaneArtwork() {
  const id = `ccc-airplane-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return <svg className="ccc-airplane-artwork" viewBox="0 0 96 64" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-upper-wing`} x1=".08" y1=".58" x2=".94" y2=".15">
        <stop stopColor="var(--ccc-display-art-metal-light)" />
        <stop offset=".65" stopColor="var(--ccc-display-art-ice)" />
        <stop offset="1" stopColor="var(--ccc-display-art-blue)" />
      </linearGradient>
      <linearGradient id={`${id}-lower-wing`} x1=".32" y1="1" x2=".8" y2="0">
        <stop stopColor="var(--ccc-display-art-depth)" />
        <stop offset=".48" stopColor="var(--ccc-display-art-blue)" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" />
      </linearGradient>
      <radialGradient id={`${id}-shadow`}>
        <stop stopColor="var(--ccc-hardware-art-black)" stopOpacity=".18" />
        <stop offset="1" stopColor="var(--ccc-hardware-art-black)" stopOpacity="0" />
      </radialGradient>
    </defs>
    <ellipse cx="51" cy="58" rx="31" ry="4" fill={`url(#${id}-shadow)`} />
    <g strokeLinejoin="round">
      <path d="M41 35.5 87 10 51 44 44 51.5Z" fill="var(--ccc-display-art-depth)" />
      <path d="M44 51.5 49 41 55 47Z" fill="var(--ccc-hardware-art-metal-shade)" />
      <path d="M9 25 87 10 41 35.5Z" fill={`url(#${id}-upper-wing)`}
        stroke="var(--ccc-display-art-blue)" strokeOpacity=".5" strokeWidth=".8" />
      <path d="M49 41 87 10 60 55Z" fill={`url(#${id}-lower-wing)`}
        stroke="var(--ccc-display-art-blue)" strokeOpacity=".7" strokeWidth=".8" />
      <path d="M10 25 87 10 49 41" fill="none" stroke="var(--ccc-hardware-art-highlight)"
        strokeOpacity=".7" strokeWidth="1.2" strokeLinecap="round" />
    </g>
  </svg>;
}
