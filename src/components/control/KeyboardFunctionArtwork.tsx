import { useId } from "react";
import type { KeyboardValues } from "../../api/client";

function Keycap({ id, legend, transform }: { id: string; legend: "fn" | "win"; transform: string }) {
  const shell = "M9 1h32a7 7 0 0 1 7 6l2 23a7 7 0 0 1-7 8H7a7 7 0 0 1-7-8L2 7a7 7 0 0 1 7-6Z";
  return <g transform={transform}>
    <path d={shell} transform="translate(0 4)" fill="var(--ccc-display-art-depth)" />
    <path d={shell} fill={`url(#${id}-metal)`} stroke="var(--ccc-display-art-metal)" strokeWidth=".85" />
    <path d="M11 5h28q4.5 0 5 4.5l1.5 16q.5 5.5-5 5.5h-31q-5.5 0-5-5.5l1.5-16Q6.5 5 11 5Z"
      fill={`url(#${id}-glass)`} />
    <path d="M11 5h28q4.5 0 5 4.5l.2 2.2c-11 5.4-24 2.6-39.3 10.1L6 9.5Q6.5 5 11 5Z"
      fill="var(--ccc-display-art-ice)" opacity=".24" />
    <path d="M9 36.5h32" stroke="var(--ccc-hardware-art-highlight)" strokeOpacity=".69" strokeWidth=".9" strokeLinecap="round" />
    <g fill="var(--ccc-hardware-art-highlight)">
      {legend === "fn"
        ? <text x="25" y="24.5" textAnchor="middle" fontSize="17" fontWeight="650">Fn</text>
        : <path d="M16.5 10h7.5v7.5h-7.5Zm9.5 0h7.5v7.5H26Zm-9.5 9.5h7.5V27h-7.5Zm9.5 0h7.5V27H26Z" />}
    </g>
  </g>;
}

export function KeyboardFunctionArtwork({ setting }: { setting: keyof KeyboardValues }) {
  const id = `ccc-keyboard-function-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const swap = setting === "fnWinSwap";
  const shackle = "M63 36v-7a8 8 0 0 1 16 0v7";

  return <svg className="ccc-keyboard-function-art" viewBox="0 0 96 64" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-metal`} x2=".82" y2="1">
        <stop stopColor="var(--ccc-display-art-metal-light)" />
        <stop offset=".6" stopColor="var(--ccc-display-art-metal)" />
        <stop offset="1" stopColor="var(--ccc-hardware-art-metal-shade)" />
      </linearGradient>
      <linearGradient id={`${id}-glass`} x1=".16" y1="1" x2=".9" y2="0">
        <stop stopColor="var(--ccc-display-art-depth)" />
        <stop offset=".64" stopColor="var(--ccc-display-art-blue)" />
        <stop offset="1" stopColor="var(--ccc-display-art-violet)" />
      </linearGradient>
      <radialGradient id={`${id}-shadow`}>
        <stop stopColor="var(--ccc-hardware-art-black)" stopOpacity=".22" />
        <stop offset="1" stopColor="var(--ccc-hardware-art-black)" stopOpacity="0" />
      </radialGradient>
    </defs>
    <ellipse cx="48" cy={swap ? 49 : 54} rx="43" ry="5" fill={`url(#${id}-shadow)`} />
    {swap ? <>
      <Keycap id={id} legend="fn" transform="translate(5 15) scale(.8)" />
      <Keycap id={id} legend="win" transform="translate(51 15) scale(.8)" />
      <path d="M22 12Q47 1 73 11m-4.5-6 4.5 6-7.5 1M74 51Q49 62 23 52m4.5 6-4.5-6 7.5-1"
        fill="none" stroke="var(--ccc-action-ink)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
    </> : <>
      <Keycap id={id} legend={setting === "fnLocked" ? "fn" : "win"} transform="translate(9 8) scale(1.12)" />
      <path d={shackle} fill="none" stroke="var(--ccc-display-art-depth)" strokeWidth="6.3" />
      <path d={shackle} fill="none" stroke={`url(#${id}-metal)`} strokeWidth="4.1" />
      <rect x="57" y="33" width="28" height="21" rx="5" fill={`url(#${id}-metal)`}
        stroke="var(--ccc-display-art-depth)" strokeWidth=".9" />
      <path d="M60.5 37q0-1.5 2-1.5h16" fill="none" stroke="var(--ccc-hardware-art-highlight)" strokeOpacity=".72" strokeLinecap="round" />
      <path d="M69.5 45a2.6 2.6 0 1 1 3 0l.6 3.7h-4.2Z" fill="var(--ccc-display-art-depth)" />
    </>}
  </svg>;
}
