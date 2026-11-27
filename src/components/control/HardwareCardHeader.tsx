import type { ReactNode } from "react";
import { Icon } from "../Icon";
import { BrandMark, HardwareArtwork } from "./HardwareArtwork";
import type { CpuArtworkBrand } from "./ProcessorArtwork";

type HardwareCardHeaderProps = (
  | { kind: "cpu"; brand: CpuArtworkBrand }
  | { kind: "gpu"; brand: "nvidia" | null }
  | { kind: "memory" | "storage"; brand?: never }
) & { title: string; model?: string; children: ReactNode };

export function HardwareCardHeader(props: HardwareCardHeaderProps) {
  const processor = props.kind === "cpu" || props.kind === "gpu";
  return <header className="ccc-hardware-identity">
    <div className="ccc-hardware-emblem">
      {props.kind === "cpu" ? <HardwareArtwork kind="cpu" brand={props.brand} /> : <HardwareArtwork kind={props.kind} />}
    </div>
    <div className="ccc-hardware-description">
      <h3>{props.title}</h3>
      {props.model && <p>{props.model}</p>}
    </div>
    {processor && <div className="ccc-hardware-mark">
      {props.brand ? <BrandMark brand={props.brand} /> : <Icon name={props.kind} />}
    </div>}
    <div className="ccc-hardware-action">{props.children}</div>
  </header>;
}
