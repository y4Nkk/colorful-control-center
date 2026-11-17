import { useId } from "react";

/** 电量决定填充体积；外壳是语义示意，不代表具体电芯型号。 */
export function BatteryArtwork({ percent, charging }: { percent: number | null; charging: boolean | null }) {
  const id = useId(), shell = `${id}-shell`, fill = `${id}-fill`, clip = `${id}-clip`;
  return <svg className="ccc-battery-artwork" viewBox="0 0 320 180" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={shell} x1="0" y1="0" x2="0" y2="1"><stop stopColor="var(--ccc-content)" /><stop offset="1" stopColor="var(--ccc-content-muted)" /></linearGradient>
      <linearGradient id={fill} x1="0" y1="0" x2="1" y2="1"><stop stopColor="currentColor" stopOpacity=".28" /><stop offset="1" stopColor="currentColor" stopOpacity=".75" /></linearGradient>
      <clipPath id={clip}><rect x="43" y="43" width="222" height="94" rx="15" /></clipPath>
    </defs>
    <ellipse cx="155" cy="158" rx="114" ry="8" fill="var(--ccc-text)" opacity=".04" />
    <rect x="272" y="64" width="16" height="52" rx="7" fill="var(--ccc-hairline)" />
    <rect x="27" y="27" width="254" height="126" rx="30" fill="var(--ccc-content-muted)" />
    <rect x="29" y="25" width="250" height="126" rx="28" fill={`url(#${shell})`} stroke="var(--ccc-hairline)" strokeWidth="1.5" />
    <rect x="42" y="40" width="224" height="98" rx="17" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
    <g clipPath={`url(#${clip})`}>
      {percent !== null && <rect data-battery-fill="true" x="43" y="43" width={222 * percent / 100} height="94" fill={`url(#${fill})`} />}
      {percent !== null && percent > 0 && <path d={`M43 44h${222 * percent / 100}`} stroke="currentColor" strokeWidth="2" opacity=".3" />}
    </g>
    <path d="M50 30h207" stroke="var(--ccc-content)" strokeWidth="2" strokeLinecap="round" opacity=".8" />
    {charging === true && <g data-battery-charging="true" transform="translate(246 123)"><circle r="17" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" /><path d="m2-10-9 12h6l-1 8L7-2H1Z" fill="currentColor" /></g>}
  </svg>;
}
