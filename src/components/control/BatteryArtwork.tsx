import { useId } from "react";

/** 电量决定填充体积；外壳是语义示意，不代表具体电芯型号。 */
export function BatteryArtwork({ percent, charging }: { percent: number | null; charging: boolean | null }) {
  const clip = useId();
  const fillWidth = 94;
  return <svg className="ccc-battery-artwork" viewBox="0 0 116 60" aria-hidden="true" focusable="false">
    <defs>
      <clipPath id={clip}><rect x="6" y="9" width={fillWidth} height="42" rx="7" /></clipPath>
    </defs>
    <rect x="2" y="5" width="102" height="50" rx="11" fill="var(--ccc-content-muted)" stroke="var(--ccc-text-muted)" strokeWidth="2" />
    <rect x="108" y="21" width="6" height="18" rx="3" fill="var(--ccc-text-muted)" />
    {percent !== null && <rect data-battery-fill="true" x="6" y="9" width={fillWidth * percent / 100} height="42"
      fill="currentColor" clipPath={`url(#${clip})`} />}
    {charging === true && <g data-battery-charging="true" transform="translate(88 39)">
      <circle r="9" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
      <path d="m1-6-5 7h4l-1 5 5-7H0Z" fill="currentColor" />
    </g>}
  </svg>;
}
