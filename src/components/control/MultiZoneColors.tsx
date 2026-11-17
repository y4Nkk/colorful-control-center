import type { CSSProperties } from "react";
import type { MultiZoneValues } from "../../api/client";
import { hexOf } from "./useKeyboardLighting";

export function MultiZoneColors({ values, label }: { values: MultiZoneValues; label: string }) {
  const zones = [...values.zones.map((color, index) => ({ name: `第${index + 1}区`, color })),
    ...(values.lightBar ? [{ name: "灯条区", color: values.lightBar }] : []),
    ...(values.cover ? [{ name: "顶盖区", color: values.cover }] : [])];
  return <div className="ccc-multi-zone-colors" aria-label={label}>
    {zones.map(({ name, color }) => <div key={name} className="ccc-multi-zone-color" style={{ "--zone-color": hexOf(color) } as CSSProperties}>
      <span className="ccc-multi-zone-swatch" aria-hidden="true" /><span>{name}</span><code>{hexOf(color)}</code>
    </div>)}
    <p className="ccc-control-caption">静态颜色预设 · 灯区排列仅作示意</p>
  </div>;
}
