import { useId } from "react";

function CoolingFan({ x, blade, metal }: { x: number; blade: string; metal: string }) {
  const opening = useId();
  return <g data-gpu-fan transform={`translate(${x} 58)`}>
    <defs><clipPath id={opening}><circle r="45" /></clipPath></defs>
    <circle r="49" fill="var(--ccc-hardware-art-ink)" stroke="var(--ccc-hardware-art-rim)" strokeWidth="1.6" />
    <g clipPath={`url(#${opening})`}>
      <circle r="46" fill="var(--ccc-hardware-art-black)" />
      {Array.from({ length: 25 }, (_, index) => <path key={index} d={`M-46 ${-43 + index * 3.6}H46`}
        stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth="1.2" opacity=".6" />)}
      {Array.from({ length: 9 }, (_, index) => <g key={index} transform={`rotate(${index * 40})`}>
        <path d="M4-11C11-22 14-34 7-44C18-43 27-37 31-29C28-17 17-10 7-7Z" fill={`url(#${blade})`} />
        <path d="M8-43C17-42 25-36 29-30" stroke="var(--ccc-hardware-art-rim)" strokeWidth=".75" fill="none" opacity=".65" />
      </g>)}
    </g>
    <circle r="45.5" fill="none" stroke="var(--ccc-hardware-art-black)" strokeWidth="1.8" />
    <circle r="13" fill={`url(#${blade})`} stroke="var(--ccc-hardware-art-rim)" strokeWidth="1" />
    <circle r="10" fill="none" stroke={`url(#${metal})`} strokeWidth=".5" opacity=".5" />
    <path d="m-5-3 5-3 5 3v6L0 6l-5-3Z" fill="none" stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth=".7" />
    <path d="M-30-36A47 47 0 0 1 30-36" stroke="var(--ccc-hardware-art-metal)" strokeWidth=".7" fill="none" opacity=".6" />
  </g>;
}

function CardBracket({ metal }: { metal: string }) {
  return <g data-gpu-bracket>
    <path d="m-12-5 13 3v119l-6 7h-7v-10h4V4h-4Z" fill={`url(#${metal})`} stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth=".6" />
    {[12, 30, 48].map(y => <path key={y} d={`M-8 ${y}h6v11h-6Z`} fill="var(--ccc-hardware-art-black)" stroke="var(--ccc-hardware-art-rim)" strokeWidth=".5" />)}
    <path d="m-8 68 6-1v9l-6 2Z" fill="var(--ccc-hardware-art-black)" />
    {Array.from({ length: 6 }, (_, i) => <path key={i} d={`m-7 ${84 + i * 3.3} 4-1v1.4l-4 1Z`} fill="var(--ccc-hardware-art-ink)" />)}
    <ellipse cx="-7" cy="1" rx="2.5" ry="1" fill="var(--ccc-hardware-art-black)" />
  </g>;
}

export function GraphicsCardArtwork({ metal }: { metal: string }) {
  const id = useId();
  const shell = `${id}-shell`, blade = `${id}-blade`;
  return <g data-desktop-gpu transform="matrix(1 .12 -.18 .88 42 33)">
    <defs>
      <linearGradient id={shell} x1="0" y1="0" x2=".7" y2="1">
        <stop stopColor="var(--ccc-hardware-art-rim)" /><stop offset=".4" stopColor="var(--ccc-hardware-art-shell)" /><stop offset="1" stopColor="var(--ccc-hardware-art-ink)" />
      </linearGradient>
      <linearGradient id={blade} x1="0" y1="0" x2=".8" y2="1">
        <stop stopColor="var(--ccc-hardware-art-shell)" /><stop offset=".4" stopColor="var(--ccc-hardware-art-ink)" /><stop offset=".85" stopColor="var(--ccc-hardware-art-black)" /><stop offset="1" stopColor="var(--ccc-hardware-art-rim)" />
      </linearGradient>
    </defs>
    <path d="M7 3 20-7h204l14 14v95l-9 12H7Z" fill="var(--ccc-hardware-art-black)" />
    <path d="M10 0 23-9h199l16 16-9 9Z" fill={`url(#${shell})`} />
    <path d="m229 16 9-9v95l-9 12Z" fill="var(--ccc-hardware-art-black)" />
    <g data-gpu-power transform="translate(191 -12)">
      <path d="M0 0h25v13H0Z" fill="var(--ccc-hardware-art-ink)" stroke="var(--ccc-hardware-art-rim)" strokeWidth=".7" />
      {Array.from({ length: 8 }, (_, i) => <rect key={i} x={3 + i % 4 * 5.5} y={2 + Math.floor(i / 4) * 5} width="3.5" height="3.5" fill="var(--ccc-hardware-art-black)" />)}
    </g>
    <path d="M12 112h190v7H12Z" fill="var(--ccc-hardware-art-edge)" />
    <path d="M49 117h19v10H49Zm25 0h111v10H74Z" fill="var(--ccc-hardware-art-gold)" />
    <g data-gpu-contacts stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth=".45">
      {Array.from({ length: 48 }, (_, i) => <path key={i} d={`M${i < 7 ? 51 + i * 2.4 : 76 + (i - 7) * 2.6} 118v9`} />)}
    </g>
    <CardBracket metal={metal} />
    <path d="M9 0h207l13 13v88l-13 13H9l-9-10V10Z" fill={`url(#${shell})`} stroke="var(--ccc-hardware-art-rim)" strokeWidth=".9" />
    <path d="m9 5 20-5h14L8 25m200-25 16 16v9L195 0M6 88l23 26H16L1 99m223-13-25 28h13l17-19" stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth="1.4" fill="none" />
    <path d="m97 1 20 25 20-25m-40 112 20-25 20 25" fill="var(--ccc-hardware-art-rim)" stroke="var(--ccc-hardware-art-shell)" strokeWidth="1" />
    <CoolingFan x={58} blade={blade} metal={metal} />
    <CoolingFan x={171} blade={blade} metal={metal} />
    {[8, 219].flatMap(x => [9, 105].map(y => <g key={`${x}-${y}`}><circle cx={x} cy={y} r="1.5" fill={`url(#${metal})`} /><path d={`m${x - .7} ${y}h1.4`} stroke="var(--ccc-hardware-art-ink)" strokeWidth=".5" /></g>))}
  </g>;
}
