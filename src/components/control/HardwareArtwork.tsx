import { useId } from "react";
import { ProcessorArtwork, type CpuArtworkBrand } from "./ProcessorArtwork";
import { GraphicsCardArtwork } from "./GraphicsCardArtwork";
import { MemoryArtwork, StorageArtwork } from "./ModuleArtwork";

export type HardwareKind = "cpu" | "gpu" | "memory" | "storage";
export type HardwareBrand = "intel" | "amd" | "nvidia";

type HardwareArtworkProps = { kind: "cpu"; brand: CpuArtworkBrand } | { kind: Exclude<HardwareKind, "cpu"> };

export function BrandMark({ brand }: { brand: HardwareBrand }) {
  return <span className={`ccc-hardware-brand is-${brand}`}>
    <img src={`/brands/${brand}.${brand === "intel" ? "png" : "svg"}`} alt={brand === "intel" ? "Intel" : brand.toUpperCase()} />
  </span>;
}

export function HardwareArtwork(props: HardwareArtworkProps) {
  const { kind } = props;
  const id = useId();
  const metal = `${id}-metal`;
  return <svg className={`ccc-hardware-art is-${kind}`} viewBox="0 0 300 190" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={metal} x1="0" y1="0" x2=".8" y2="1">
        <stop stopColor="var(--ccc-hardware-art-highlight)" />
        <stop offset=".3" stopColor="var(--ccc-hardware-art-metal)" />
        <stop offset=".55" stopColor="var(--ccc-hardware-art-highlight)" />
        <stop offset="1" stopColor="var(--ccc-hardware-art-metal-shade)" />
      </linearGradient>
      <filter id={`${id}-shadow`} x="-.2" y="-1" width="1.4" height="3"><feGaussianBlur stdDeviation="4" /></filter>
    </defs>
    <ellipse cx="150" cy="163" rx={kind === "cpu" ? 76 : 108} ry="7" fill="var(--ccc-hardware-art-shadow)" opacity=".15" filter={`url(#${id}-shadow)`} />
    {props.kind === "cpu" && <ProcessorArtwork metal={metal} brand={props.brand} />}
    {kind === "gpu" && <GraphicsCardArtwork metal={metal} />}
    {kind === "memory" && <MemoryArtwork metal={metal} />}
    {kind === "storage" && <StorageArtwork metal={metal} />}
  </svg>;
}
