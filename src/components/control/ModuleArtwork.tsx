import { useId } from "react";

export function MemoryArtwork({ metal }: { metal: string }) {
  const shell = useId();
  return <g data-memory-heatsink transform="matrix(1 -.17 .18 .96 24 66)">
    <defs><linearGradient id={shell} x1="0" y1="0" x2="0" y2="1">
      <stop stopColor="var(--ccc-hardware-art-rim)" /><stop offset=".3" stopColor="var(--ccc-hardware-art-shell)" /><stop offset="1" stopColor="var(--ccc-hardware-art-ink)" />
    </linearGradient></defs>
    <path d="m0 8 7-6h221l5 5v65H0Z" fill="var(--ccc-hardware-art-black)" />
    <path d="M1 6h230v65H139v-8h-7v8H1Z" fill="var(--ccc-hardware-art-chip)" />
    {Array.from({ length: 65 }, (_, i) => i >= 36 && i <= 38 ? null : <rect key={i}
      x={6 + i * 3.4} y="63" width="2.2" height="8" fill="var(--ccc-hardware-art-gold)" />)}
    <path d="M8 2 15-4h207l7 6Z" fill="var(--ccc-hardware-art-rim)" />
    <path d="M5 3h225v51H5V38H1V26h4Z" fill={`url(#${shell})`} stroke="var(--ccc-hardware-art-rim)" strokeWidth=".6" />
    <g data-memory-fins>
      {Array.from({ length: 28 }, (_, i) => <g key={i}>
        <path d={`M${9 + i * 8} 2v${i > 8 && i < 19 ? 9 : 47}`} stroke="var(--ccc-hardware-art-black)" strokeWidth="2.4" />
        <path d={`M${11 + i * 8} 2v${i > 8 && i < 19 ? 9 : 47}`} stroke="var(--ccc-hardware-art-rim)" strokeWidth=".8" />
      </g>)}
    </g>
    <path d="M74 12h87l-5 35H80Z" fill="var(--ccc-hardware-art-ink)" stroke="var(--ccc-hardware-art-rim)" strokeWidth=".7" />
    <path d="M82 16h70v22H82Z" fill="var(--ccc-hardware-art-black)" />
    <text className="ccc-art-etching" x="117" y="29" textAnchor="middle" fontSize="9" letterSpacing="1.1" fontWeight="600" fill="var(--ccc-hardware-art-gold)">MEMORY</text>
    <path d="M100 43h34" stroke={`url(#${metal})`} strokeWidth=".8" opacity=".6" />
    <path d="M7 54h220m-222 4h223" stroke="var(--ccc-hardware-art-engraving)" strokeWidth=".5" opacity=".4" />
    <path d="M0 24h5v14H0m232-14h-5v14h5" fill="var(--ccc-hardware-art-highlight)" opacity=".6" />
    {Array.from({ length: 14 }, (_, i) => <g key={i} fill={`url(#${metal})`}>
      <rect x={10 + i * 15} y="57" width="1.8" height="1.5" /><rect x={13 + i * 15} y="57" width="1.8" height="1.5" />
    </g>)}
  </g>;
}

export function StorageArtwork({ metal }: { metal: string }) {
  return <g data-m2-drive transform="matrix(1 .13 -.22 .95 42 56)">
    <path d="M0 2h219v58h-12v-7h-7v7H0v-16a7 7 0 0 0 0-14Z" fill="var(--ccc-hardware-art-edge)" transform="translate(0 3)" />
    <path d="M0 0h219v57h-12v-7h-7v7H0V37a7 7 0 0 0 0-14Z" fill="var(--ccc-hardware-art-black)" stroke="var(--ccc-hardware-art-rim)" strokeWidth=".6" />
    <path data-m2-mount d="M0 23a7 7 0 0 1 0 14" stroke="var(--ccc-hardware-art-gold)" strokeWidth="2.5" fill="none" />
    {Array.from({ length: 18 }, (_, i) => <g key={i}>
      <rect x="210" y={3 + i * 2.8} width="12" height="1.7" fill="var(--ccc-hardware-art-gold)" />
      <rect x={12 + i * 10.3} y="3" width="3.3" height="2" fill={`url(#${metal})`} />
      <rect x={12 + i * 10.3} y="52" width="2.8" height="2" fill={`url(#${metal})`} />
    </g>)}
    <path data-m2-key d="M207 46h15v5h-15Z" fill="var(--ccc-hardware-art-black)" />
    {[14, 65, 116].map(x => <rect key={x} x={x} y="9" width="40" height="38" rx="1.5" fill="var(--ccc-hardware-art-chip)" />)}
    <rect x="168" y="9" width="28" height="37" rx="2" fill={`url(#${metal})`} />
    <path d="M12 8h150q6 0 6 6v28q0 6-6 6H12q-5 0-5-5V13q0-5 5-5Z" fill="var(--ccc-hardware-art-label)" />
    <text className="ccc-art-etching" x="17" y="27" fontSize="12" fontWeight="600" letterSpacing="1.5" fill="var(--ccc-hardware-art-highlight)">STORAGE</text>
    <path d="M17 32h76" stroke="var(--ccc-hardware-art-label-accent)" strokeWidth="2" />
    <path d="M17 37h46m-46 4h30m75-26h32m-32 4h26" stroke="var(--ccc-hardware-art-metal-shade)" strokeWidth=".9" />
    <g stroke="var(--ccc-hardware-art-highlight)" strokeWidth=".7" opacity=".7">
      {[0, 2, 5, 6, 9, 12, 13, 15, 18, 21, 23, 24].map(x => <path key={x} d={`M${127 + x} 34v7`} />)}
    </g>
    {Array.from({ length: 6 }, (_, i) => <g key={i}>
      <rect x="195" y={10 + i * 5.2} width="3" height="2.2" fill={`url(#${metal})`} />
      <rect x="200" y={12 + i * 5.2} width="2" height="2.2" fill="var(--ccc-hardware-art-gold)" />
    </g>)}
    <circle cx="5" cy="5" r="1.6" fill="var(--ccc-hardware-art-gold)" />
    <circle cx="5" cy="51" r="1.6" fill="var(--ccc-hardware-art-gold)" />
  </g>;
}
