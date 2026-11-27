import { useId } from "react";

const hueColor = (hue: number) => `hsl(${hue} 100% 50%)`;
const ringPoint = (degrees: number) => {
  const angle = degrees * Math.PI / 180;
  return `${50 + 40 * Math.sin(angle)} ${50 - 40 * Math.cos(angle)}`;
};

const hueArcs = Array.from({ length: 180 }, (_, index) => {
  const hue = index * 2;
  return <path key={hue} d={`M${ringPoint(hue - 1)}A40 40 0 0 1 ${ringPoint(hue + 3)}`}
    stroke={hueColor(hue + 1)} />;
});

export function HueSpectrum({ circular = false }: { circular?: boolean }) {
  const gradient = useId();
  if (circular) return <svg className="ccc-rgb-spectrum" viewBox="0 0 100 100" aria-hidden="true" focusable="false">
    <g fill="none" strokeWidth="20">{hueArcs}</g>
  </svg>;
  return <svg className="ccc-rgb-spectrum" viewBox="0 0 360 12" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <defs><linearGradient id={gradient} colorInterpolation="sRGB">
      {[0, 60, 120, 180, 240, 300, 360].map(hue => <stop key={hue} offset={hue / 360} stopColor={hueColor(hue)} />)}
    </linearGradient></defs>
    <rect width="360" height="12" fill={`url(#${gradient})`} />
  </svg>;
}

export function SaturationSpectrum({ hue }: { hue: number }) {
  const id = useId();
  return <svg className="ccc-rgb-spectrum" viewBox="0 0 100 50" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-saturation`}>
        <stop stopColor="white" /><stop offset="1" stopColor={hueColor(hue)} />
      </linearGradient>
      <linearGradient id={`${id}-value`} x2="0" y2="1" gradientUnits="objectBoundingBox">
        <stop stopColor="black" stopOpacity="0" /><stop offset="1" stopColor="black" />
      </linearGradient>
    </defs>
    <rect width="100" height="50" fill={`url(#${id}-saturation)`} />
    <rect width="100" height="50" fill={`url(#${id}-value)`} />
  </svg>;
}
