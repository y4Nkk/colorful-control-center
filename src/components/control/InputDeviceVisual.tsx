import { useId } from "react";

function TouchpadArtwork({ id }: { id: string }) {
  const shell = "M26 13.5h44q3.5 0 5.2 3.2l13 25.8q4.1 8-4.9 8H12.7q-9 0-4.9-8l13-25.8q1.7-3.2 5.2-3.2Z";
  return <>
    <defs>
      <linearGradient id={`${id}-glass`} x1=".14" y1="1" x2=".86" y2="0">
        <stop stopColor="var(--ccc-display-art-depth)" />
        <stop offset=".66" stopColor="var(--ccc-display-art-blue)" />
        <stop offset="1" stopColor="var(--ccc-display-art-violet)" />
      </linearGradient>
      <linearGradient id={`${id}-reflection`} x1=".17" y1="1" x2=".9" y2="0">
        <stop stopColor="var(--ccc-display-art-ice)" stopOpacity="0" />
        <stop offset="1" stopColor="var(--ccc-display-art-ice)" stopOpacity=".66" />
      </linearGradient>
    </defs>
    <ellipse cx="48" cy="55" rx="44" ry="6" fill={`url(#${id}-shadow)`} />
    <path d={shell} transform="translate(0 3)" fill="var(--ccc-display-art-depth)" />
    <path d={shell} fill={`url(#${id}-metal)`} stroke="var(--ccc-display-art-metal)" strokeWidth=".85" />
    <path d="M27 17h42q2.8 0 4 2.8l11.1 22.8q2 4.2-3.4 4.2H15.3q-5.4 0-3.4-4.2L23 19.8q1.2-2.8 4-2.8Z"
      fill={`url(#${id}-glass)`} />
    <path d="M27 17h42q2.8 0 4 2.8l2.2 4.5c-12.9 7.8-24.9 4.3-41.7 8.7-7.2 1.9-13.7 4.8-20.1 7.8L23 19.8q1.2-2.8 4-2.8Z"
      fill={`url(#${id}-reflection)`} />
    <path d="M16 50.5h64" fill="none" stroke="var(--ccc-hardware-art-highlight)" strokeOpacity=".78" strokeWidth="1" />
    <g fill="none" stroke="var(--ccc-display-art-ice)" strokeWidth="1.3">
      <ellipse cx="48" cy="33" rx="14" ry="6" opacity=".64" />
      <ellipse cx="48" cy="33" rx="7" ry="3.1" />
    </g>
  </>;
}

function MicrophoneArtwork({ id }: { id: string }) {
  const cradle = "M27.5 28.5v2a20.5 20.5 0 0 0 41 0v-2";
  const metal = `url(#${id}-metal)`;
  return <>
    <defs>
      <linearGradient id={`${id}-grille`} x2=".86" y2=".55">
        <stop stopColor="var(--ccc-display-art-ice)" />
        <stop offset=".5" stopColor="var(--ccc-display-art-blue)" />
        <stop offset="1" stopColor="var(--ccc-display-art-depth)" />
      </linearGradient>
    </defs>
    <ellipse cx="48" cy="59" rx="31" ry="4.5" fill={`url(#${id}-shadow)`} />
    <path d="M26 54.5h44v1.8c0 2.1-9.9 3.8-22 3.8s-22-1.7-22-3.8Z" fill="var(--ccc-display-art-depth)" />
    <ellipse cx="48" cy="54.5" rx="22" ry="3.7" fill={metal} />
    <rect x="45" y="47" width="6" height="8.5" rx="2.3" fill={metal} stroke="var(--ccc-display-art-metal)" strokeWidth=".8" />
    <g fill="none" strokeLinecap="round">
      <path d={cradle} stroke="var(--ccc-display-art-depth)" strokeWidth="4.8" />
      <path d={cradle} stroke={metal} strokeWidth="2.8" />
      <path d="M27.5 27.5H35m26 0h7.5" stroke="var(--ccc-display-art-metal)" strokeWidth="5.5" />
    </g>
    <rect x="33" y="4.5" width="30" height="39" rx="15" fill={metal} stroke="var(--ccc-display-art-metal)" strokeWidth="1" />
    <rect x="36" y="7.5" width="24" height="31.5" rx="12" fill={`url(#${id}-grille)`} />
    <path d="M44 13.5h8m-10 4h12m-13 4h14m-14 4h14m-13 4h12m-10 4h8" fill="none"
      stroke="var(--ccc-display-art-depth)" strokeOpacity=".79" strokeWidth="1.45" strokeLinecap="round" />
    <path d="M39 18a9 9 0 0 1 9-9m-4 32h8" fill="none" stroke="var(--ccc-hardware-art-highlight)"
      strokeOpacity=".72" strokeWidth="1" strokeLinecap="round" />
  </>;
}

export function InputDeviceVisual({ kind }: { kind: "touchpad" | "microphone" }) {
  const id = `ccc-input-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return <svg className="ccc-device-art" viewBox="0 0 96 64" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-metal`} x2=".82" y2="1">
        <stop stopColor="var(--ccc-display-art-metal-light)" />
        <stop offset="1" stopColor="var(--ccc-display-art-metal)" />
      </linearGradient>
      <radialGradient id={`${id}-shadow`}>
        <stop stopColor="var(--ccc-hardware-art-black)" stopOpacity=".27" />
        <stop offset="1" stopColor="var(--ccc-hardware-art-black)" stopOpacity="0" />
      </radialGradient>
    </defs>
    {kind === "touchpad" ? <TouchpadArtwork id={id} /> : <MicrophoneArtwork id={id} />}
  </svg>;
}
