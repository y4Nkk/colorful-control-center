const INTEL_LID = "M25 10H79Q92 10 92 23V48Q92 53 98 57Q101 60 101 66V94Q101 101 95 105L92 109V121Q92 134 79 134H25Q12 134 12 121V109L6 105Q3 101 3 94V66Q3 60 9 56Q12 53 12 48V23Q12 10 25 10Z";
const AMD_LID = "M6 6Q12 2 22 4V14Q22 18 26 18H39Q43 18 43 14V4H51V14Q51 18 55 18H74Q78 18 78 14V4Q96 2 98 17V22H90Q86 22 86 26V39Q86 43 90 43H98V63H90Q86 63 86 67V80Q86 84 90 84H98V89Q96 100 78 98V90Q78 86 74 86H55Q51 86 51 90V98H43V90Q43 86 39 86H26Q22 86 22 90V98Q4 100 2 86V80H10Q14 80 14 76V66Q14 62 10 62H2V43H10Q14 43 14 39V26Q14 22 10 22H2V17Q2 10 6 6Z";

export type CpuArtworkBrand = "intel" | "amd" | null;

function PackageComponents({ amd }: { amd: boolean }) {
  return <g>
    {(amd ? [8, 27, 50, 73, 96] : [5, 139]).map(y =>
      Array.from({ length: amd ? 3 : 12 }, (_, i) => <rect key={`${y}-${i}`} x={amd ? 4 + i * 4 : 22 + i * 5}
        y={y} width={amd ? 2 : 2.2} height="1.6" fill="var(--ccc-hardware-art-gold)" />))}
    {[31, 37, 43, 68, 74, 80, 104, 110].filter(y => !amd || y < 96).map(y => <g key={y}>
      <rect x="5" y={y} width="4" height="2" fill="var(--ccc-hardware-art-chip)" />
      <rect x="5" y={y} width="1" height="2" fill="var(--ccc-hardware-art-metal)" />
      <rect x="96" y={y} width="3" height="2" fill="var(--ccc-hardware-art-gold)" />
    </g>)}
  </g>;
}

export function ProcessorArtwork({ metal, brand }: { metal: string; brand: CpuArtworkBrand }) {
  const amd = brand === "amd";
  const label = amd ? "RYZEN" : brand === "intel" ? "CORE" : "CPU";
  const lid = amd ? AMD_LID : INTEL_LID;
  const height = amd ? 104 : 144;
  return <g data-package={amd ? "am5" : label === "CORE" ? "lga1700" : "generic"} transform={amd ? "matrix(1.3 .3 -.66 .79 117 21)" : "matrix(1.24 .27 -.6 .83 128 11)"}>
    <rect y="5" width="104" height={height} rx="3" fill="var(--ccc-hardware-art-edge)" />
    <path d={`M0 0H104V${height}H0Z`} fill="var(--ccc-hardware-art-board)" />
    <path d={`M1 1H103V${height - 1}H1Z`} stroke="var(--ccc-hardware-art-engraving)" fill="none" strokeWidth=".6" />
    <PackageComponents amd={amd} />
    <path d={lid} fill="var(--ccc-hardware-art-metal-shade)" transform="translate(0 4)" />
    <path d={lid} fill={`url(#${metal})`} stroke="var(--ccc-hardware-art-highlight)" strokeWidth=".8" />
    {!amd && <rect x="17" y="15" width="70" height="114" rx="12" stroke="var(--ccc-hardware-art-highlight)" fill="none" strokeWidth="1.4" opacity=".75" />}
    <g fill="var(--ccc-hardware-art-ink)" opacity=".7" className="ccc-art-etching">
      <text x={amd ? 23 : 28} y={amd ? 42 : 49} fontSize={amd ? 14 : 10} fontWeight="600">{label}</text>
      <path d={amd ? "M24 49h47m-47 5h37m-37 5h44m-44 5h28" : "M29 57h42m-42 5h26m-26 5h35"} stroke="var(--ccc-hardware-art-ink)" strokeWidth="1" opacity=".45" />
      <path d={amd ? "M25 71h11v11H25Zm3 3h5v5h-5Z" : "M29 93h12v12H29Zm3 3h6v6h-6Z"} fillRule="evenodd" />
      <path d={amd ? "M43 74h21m-21 4h15" : "M47 96h19m-19 4h14"} stroke="var(--ccc-hardware-art-ink)" strokeWidth="1" opacity=".4" />
    </g>
    <path d={amd ? "m6 7 5 0-5 5Z" : "m5 134 5 5H5Z"} fill="var(--ccc-hardware-art-gold)" />
    <path d={amd ? "M48 0a4 4 0 0 0 8 0" : "M32 0a3 3 0 0 0 6 0m40 0a3 3 0 0 0 6 0"} stroke="var(--ccc-hardware-art-highlight)" strokeWidth="1.2" fill="none" />
  </g>;
}
