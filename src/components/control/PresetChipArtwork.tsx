import { useId, useLayoutEffect, useRef, useState } from "react";
import { MODE_ARTWORK_BY_MODE } from "./ModeArtwork";

function ChipDefinitions({ id }: { id: string }) {
  return <defs>
    <linearGradient id={`${id}-board`} x2=".85" y2="1">
      <stop stopColor="var(--ccc-preset-steel-high)" />
      <stop offset=".26" stopColor="var(--ccc-preset-steel-mid)" />
      <stop offset=".59" stopColor="var(--ccc-preset-steel-low)" />
      <stop offset="1" stopColor="var(--ccc-preset-steel-mid)" />
    </linearGradient>
    <linearGradient id={`${id}-edge`} x2="1" y2=".48">
      <stop stopColor="var(--ccc-preset-steel-mid)" />
      <stop offset=".48" stopColor="var(--ccc-preset-steel-low)" />
      <stop offset="1" stopColor="var(--ccc-preset-steel-deep)" />
    </linearGradient>
    <linearGradient id={`${id}-substrate`} x2="1" y2="1">
      <stop stopColor="var(--ccc-preset-steel-deep)" />
      <stop offset=".67" stopColor="var(--ccc-preset-steel-low)" />
      <stop offset="1" stopColor="var(--ccc-preset-steel-deep)" />
    </linearGradient>
    <linearGradient id={`${id}-die`} x2="1" y2="1">
      <stop className="ccc-preset-chip-paint" stopColor="var(--ccc-preset-light)" />
      <stop className="ccc-preset-chip-paint" offset=".40" stopColor="var(--ccc-preset-mid)" />
      <stop className="ccc-preset-chip-paint" offset=".73" stopColor="var(--ccc-preset-low)" />
      <stop className="ccc-preset-chip-paint" offset="1" stopColor="var(--ccc-preset-deep)" />
    </linearGradient>
    <linearGradient id={`${id}-die-edge`} x2="1" y2=".32">
      <stop className="ccc-preset-chip-paint" stopColor="var(--ccc-preset-low)" />
      <stop className="ccc-preset-chip-paint" offset=".57" stopColor="var(--ccc-preset-deep)" />
      <stop className="ccc-preset-chip-paint" offset="1" stopColor="var(--ccc-preset-low)" />
    </linearGradient>
    <linearGradient id={`${id}-reflection`} x2=".75" y2="1">
      <stop stopColor="var(--ccc-preset-glint)" stopOpacity=".66" />
      <stop offset="1" stopColor="var(--ccc-preset-glint)" stopOpacity="0" />
    </linearGradient>
    <linearGradient id={`${id}-scan`} x1="-30" y1="-9" x2="20" y2="5" gradientUnits="userSpaceOnUse">
      <stop stopColor="var(--ccc-preset-glint)" stopOpacity="0" />
      <stop offset=".43" stopColor="var(--ccc-preset-glint)" stopOpacity=".24" />
      <stop offset=".64" stopColor="var(--ccc-preset-glint)" stopOpacity=".88" />
      <stop offset=".73" stopColor="var(--ccc-preset-glint)" stopOpacity=".4" />
      <stop offset="1" stopColor="var(--ccc-preset-glint)" stopOpacity="0" />
    </linearGradient>
    <radialGradient id={`${id}-ambient`}>
      <stop className="ccc-preset-chip-paint" stopColor="var(--ccc-preset-mid)" stopOpacity=".32" />
      <stop className="ccc-preset-chip-paint" offset="1" stopColor="var(--ccc-preset-mid)" stopOpacity="0" />
    </radialGradient>
    <radialGradient id={`${id}-shadow`}>
      <stop stopColor="var(--ccc-preset-shadow)" stopOpacity=".35" />
      <stop offset="1" stopColor="var(--ccc-preset-shadow)" stopOpacity="0" />
    </radialGradient>
    <linearGradient id={`${id}-grid-fade`}>
      <stop stopColor="var(--ccc-color-white)" stopOpacity="0" /><stop offset=".23" stopColor="var(--ccc-color-white)" stopOpacity=".8" />
      <stop offset=".77" stopColor="var(--ccc-color-white)" stopOpacity=".8" /><stop offset="1" stopColor="var(--ccc-color-white)" stopOpacity="0" />
    </linearGradient>
    <mask id={`${id}-grid-mask`}>
      <rect width="600" height="250" fill={`url(#${id}-grid-fade)`} />
    </mask>
    <pattern id={`${id}-dots`} width="12" height="12" patternUnits="userSpaceOnUse">
      <circle cx="1" cy="1" r=".7" fill="var(--ccc-preset-grid)" />
    </pattern>
    <path id={`${id}-die-shape`} d="M-53-69H53l16 16V53L53 69H-53l-16-16V-53Z" />
    <clipPath id={`${id}-die-clip`}><use href={`#${id}-die-shape`} /></clipPath>
    <g id={`${id}-pins`} fill="none" strokeLinecap="round" strokeLinejoin="round">
      <path d="M-76-92h28l10 10h17v9m-34-19-9 9h-17m77-9h26l9 10h15m6-9h22l10 10"
        stroke="var(--ccc-preset-steel-high)" strokeOpacity=".66" strokeWidth="1.25" />
      <path d="M-78-82v7m7-7v7m7-7v7m7-7v7m44-17v10m9-10v10m9-10v10m9-10v10m46-1v7m7-7v7m7-7v7"
        stroke="var(--ccc-preset-glint)" strokeOpacity=".68" strokeWidth="1.7" />
      <path d="M-87-89h3m169 0h3" stroke="var(--ccc-preset-light)" strokeOpacity=".72" strokeWidth="2.2" />
    </g>
    <g id={`${id}-etching`} fill="none" stroke="var(--ccc-preset-glint)" strokeWidth=".9" strokeLinecap="round" strokeLinejoin="round">
      <path d="M-47-51h19l10 10v10m-31-12h18l6 6v8m-27-4h15l6 6v13m-23-8h12l5 5v11m26-50v16l6 6v9" />
      <circle cx="-47" cy="-51" r="1.35" /><circle cx="-52" cy="-33" r="1.35" /><circle cx="-54" cy="-22" r="1.35" />
    </g>
  </defs>;
}

function ChipBoard({ id }: { id: string }) {
  return <>
    <g mask={`url(#${id}-grid-mask)`}>
      <rect x="46" y="27" width="508" height="192" fill={`url(#${id}-dots)`} />
      <path d="M40 116H560M300 10V233" stroke="var(--ccc-preset-grid)" strokeDasharray="2 6" />
      <path d="M73 111v10m-5-5h10m449-5v10m-5-5h10" stroke="var(--ccc-preset-grid)" fill="none" />
    </g>
    <ellipse cx="300" cy="142" rx="221" ry="106" fill={`url(#${id}-ambient)`} />
    <ellipse cx="300" cy="205" rx="187" ry="35" fill={`url(#${id}-shadow)`} />
    <ellipse cx="300" cy="208" rx="103" ry="17" fill={`url(#${id}-shadow)`} />
    <path d="M124.66 122.48 286.51 200.24H313.49L475.34 122.48v11L313.49 211.24H286.51L124.66 133.48Z"
      fill={`url(#${id}-edge)`} />
    <path d="M125.7 129.5 286.51 206.6h26.98l160.81-77.1" fill="none" stroke="var(--ccc-preset-glint)" strokeOpacity=".22" />
    <g transform="matrix(.843 .405 -.843 .405 300 116)">
      <path d="M-96-112H96L112-96V96L96 112H-96l-16-16V-96Z" fill={`url(#${id}-board)`}
        stroke="var(--ccc-preset-glint)" strokeOpacity=".81" strokeWidth="1.3" />
      <path d="M-86-99H86l13 13V86L86 99H-86l-13-13V-86Z" fill={`url(#${id}-substrate)`}
        stroke="var(--ccc-preset-steel-high)" />
      <path d="M-91-105H91l14 14V91L91 105H-91l-14-14V-91Z" fill="none" stroke="var(--ccc-preset-glint)" strokeOpacity=".32" />
      <use href={`#${id}-pins`} /><use href={`#${id}-pins`} transform="rotate(90)" />
      <use href={`#${id}-pins`} transform="rotate(180)" /><use href={`#${id}-pins`} transform="rotate(270)" />
      <path d="M-62-79H62l17 17V62L62 79H-62l-17-17V-62Z" fill="var(--ccc-preset-steel-deep)"
        stroke="var(--ccc-preset-steel-high)" strokeOpacity=".42" />
    </g>
  </>;
}

function ChipDie({ id }: { id: string }) {
  return <>
    <path className="ccc-preset-chip-trace" d="M125.7 129.5 286.51 206.6h26.98l160.81-77.1"
      fill="none" stroke="var(--ccc-preset-light)" strokeWidth="2" />
    <g className="ccc-preset-chip-die">
      <path d="M197.15 116.48 286.51 159.41H313.49L402.85 116.48v8L313.49 167.41H286.51L197.15 124.48Z"
        fill={`url(#${id}-die-edge)`} />
      <g transform="matrix(.843 .405 -.843 .405 300 110)">
        <use href={`#${id}-die-shape`} fill={`url(#${id}-die)`}
          stroke="var(--ccc-preset-light)" strokeWidth="1.7" />
        <g clipPath={`url(#${id}-die-clip)`}>
          <path d="M-70-69H64L-68 53H-70Z" fill={`url(#${id}-reflection)`} opacity=".58" />
          <path d="M-63 60 62-65h8L-55 60Z" fill="var(--ccc-preset-glint)" opacity=".12" />
        </g>
        <path d="M-49-62H49l13 13V49L49 62H-49l-13-13V-49Z" fill="none"
          stroke="var(--ccc-preset-glint)" strokeOpacity=".54" strokeWidth="1.15" />
        <g opacity=".65">
          <use href={`#${id}-etching`} /><use href={`#${id}-etching`} transform="rotate(90)" />
          <use href={`#${id}-etching`} transform="rotate(180)" /><use href={`#${id}-etching`} transform="rotate(270)" />
        </g>
        <rect x="-23" y="-23" width="46" height="46" rx="5"
          fill="var(--ccc-preset-deep)" fillOpacity=".44" stroke="var(--ccc-preset-glint)" strokeOpacity=".8" strokeWidth="1.4" />
        <rect x="-17" y="-17" width="34" height="34" rx="3" fill={`url(#${id}-die)`} opacity=".83" />
        <path d="M-10-10H10V10H-10Z" fill="none" stroke="var(--ccc-preset-glint)" strokeOpacity=".63" strokeWidth=".85" />
        <path d="M-8-3H8M-8 0H8M-8 3H8" fill="none" stroke="var(--ccc-preset-glint)" strokeOpacity=".77" strokeWidth=".65" />
        <g clipPath={`url(#${id}-die-clip)`}>
          <path className="ccc-preset-chip-sweep" d="M-34-76h86L6 76h-86Z" fill={`url(#${id}-scan)`} />
        </g>
      </g>
    </g>
  </>;
}

export function PresetChipArtwork({ mode, active }: { mode?: number; active: boolean }) {
  const id = `ccc-preset-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const artwork = useRef<SVGSVGElement>(null);
  const palette = (mode === undefined ? undefined : MODE_ARTWORK_BY_MODE[mode]) ?? "neutral";
  const [animation, setAnimation] = useState({ mode, palette, playing: false });
  if (animation.mode !== mode || (!active && animation.playing)) {
    setAnimation({ mode, palette, playing: active && palette !== "neutral" && animation.palette !== "neutral" });
  }

  useLayoutEffect(() => {
    if (!animation.playing) return;
    const lift = artwork.current?.querySelector(".ccc-preset-chip-die")?.getAnimations()[0];
    const settle = () => setAnimation(current => current === animation ? { ...current, playing: false } : current);
    // 零时长动效可能没有结束事件，须在绘制前退出。
    if (!lift?.effect?.getComputedTiming().activeDuration) { settle(); return; }
    let listening = true;
    const finish = () => { if (listening) settle(); };
    void lift.finished.then(finish, finish);
    return () => { listening = false; };
  }, [animation]);

  return <svg ref={artwork} className="ccc-preset-chip" data-preset-palette={palette} data-switching={animation.playing || undefined}
    width="520" height="250" viewBox="40 0 520 250" aria-hidden="true" focusable="false">
    <ChipDefinitions id={id} />
    <ChipBoard id={id} />
    {/* 保留渐变定义以便从正在显示的颜色过渡，只有芯片动效层按模式重播。 */}
    <ChipDie key={mode} id={id} />
  </svg>;
}
