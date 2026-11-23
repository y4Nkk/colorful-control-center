import { useId, useLayoutEffect, useRef, useState } from "react";
import { MODE_ARTWORK_BY_MODE } from "./ModeArtwork";

export function presetPalette(mode: number | null | undefined) {
  return (mode == null ? undefined : MODE_ARTWORK_BY_MODE[mode]) ?? "neutral";
}

type Paint = { body: string; edge: string };

function Eco({ body, edge }: Paint) {
  return <>
    <path d="M95 131c42-27 90-33 130-20 26 8 45 23 83 27-46 28-94 37-138 21-27-10-43-23-75-28Z"
      fill={body} opacity=".42" />
    <path d="M125 165C186 157 191 80 274 57c39-11 65-5 103-20-23 22-24 70-58 96-42 33-92 29-126 17-22 11-42 16-68 15Z" fill={body} />
    <path d="M127 165c34-3 62-15 96-40 40-29 67-43 115-60m-120 64c25-4 55-1 80-11m-56-7c7-14 16-25 27-34"
      fill="none" stroke={edge} strokeWidth="1.65" strokeLinecap="round" />
    <path d="M113 130c32-11 63-12 91-4m21 53c31 4 67 1 91-8" fill="none" stroke={body} strokeOpacity=".45" strokeWidth="1.2" strokeLinecap="round" />
  </>;
}

function Quiet({ body, edge }: Paint) {
  return <>
    <path d="M270 31c-23 8-35 32-29 54 8 26 34 39 59 31-12 26-44 39-71 27-29-13-42-46-30-75 12-28 42-43 71-37Z" fill={body} />
    <path d="M210 79c-6 23 6 46 26 55" fill="none" stroke={edge} strokeWidth="1.6" strokeLinecap="round" />
    <path d="M87 144c42-5 77-3 113 6 46 11 81 13 121 2 27-8 48-10 76-7M122 163c37-2 60 4 95 10 39 7 83 2 122-9M185 182c34 6 59 8 94 3"
      fill="none" stroke={body} strokeWidth="1.65" strokeOpacity=".6" strokeLinecap="round" />
    <path d="M318 77v14m-7-7h14" fill="none" stroke={body} strokeWidth="1.45" strokeLinecap="round" />
    <circle cx="299" cy="57" r="2.2" fill={body} />
  </>;
}

function Entertainment({ body, edge }: Paint) {
  return <>
    <ellipse cx="240" cy="106" rx="147" ry="36" transform="rotate(-15 240 106)"
      fill="none" stroke={body} strokeWidth="3" opacity=".58" />
    <circle cx="240" cy="100" r="59" fill={body} />
    <path d="M193 93c5-18 17-30 34-36m-19 58c16-22 46-36 76-32m-63 56c19-18 42-29 70-30"
      fill="none" stroke={edge} strokeWidth="1.6" strokeOpacity=".68" strokeLinecap="round" />
    <g transform="rotate(-15 240 106)" fill="none">
      <path d="M93 106a147 36 0 0 0 294 0" stroke={body} strokeWidth="8" strokeLinecap="round" />
      <path d="M97 109a145 34 0 0 0 286 0" stroke={edge} strokeWidth="1.45" />
    </g>
    <circle cx="106" cy="77" r="4" fill={body} />
    <path d="M352 143v14m-7-7h14" fill="none" stroke={body} strokeWidth="1.6" strokeLinecap="round" />
  </>;
}

function Performance({ body, edge }: Paint) {
  return <>
    <path d="M91 141c52-7 88-30 126-53 42-26 80-43 127-44-42 13-69 39-99 59-48 32-101 44-154 38Z"
      fill={body} opacity=".44" />
    <path d="M98 163c57-8 106-32 151-68 43-35 83-55 150-58-34 14-51 36-75 56-58 49-136 77-226 70Z" fill={body} />
    <path d="M112 162c65-7 117-31 169-67 32-22 53-37 86-48-34 22-44 40-78 62-55 36-119 53-177 53Z"
      fill={edge} opacity=".84" />
    <path d="M186 176c47-8 84-25 126-49 25-15 50-23 78-25-23 9-39 21-58 34-37 26-83 40-146 40Z"
      fill={body} opacity=".68" />
    <path d="M100 115c30-9 58-22 80-37m149 77c22-12 44-21 65-24" fill="none" stroke={body} strokeWidth="1.7" strokeOpacity=".57" strokeLinecap="round" />
  </>;
}

function Neutral({ body }: Paint) {
  return <path d="M88 105c49-15 96-17 146-3 49 14 100 15 158-4M118 122c40-7 72-6 115 5 42 11 81 11 129 1M165 143c24 0 47 4 69 9 28 6 54 7 83 2"
    fill="none" stroke={body} strokeWidth="2" strokeOpacity=".56" strokeLinecap="round" />;
}

const drawings = { eco: Eco, quiet: Quiet, entertainment: Entertainment, performance: Performance, neutral: Neutral };

export function PresetArtwork({ mode, active }: { mode?: number; active: boolean }) {
  const id = `ccc-preset-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const drawing = useRef<SVGGElement>(null);
  const palette = presetPalette(mode);
  const [animation, setAnimation] = useState({ mode, palette, playing: false });
  if (animation.mode !== mode || (!active && animation.playing)) {
    setAnimation({ mode, palette, playing: active && palette !== "neutral" && animation.palette !== "neutral" });
  }

  useLayoutEffect(() => {
    if (!animation.playing) return;
    const reveal = drawing.current?.getAnimations()[0];
    const settle = () => setAnimation(current => current === animation ? { ...current, playing: false } : current);
    // 零时长动效可能没有结束事件，须在绘制前退出。
    if (!reveal?.effect?.getComputedTiming().activeDuration) { settle(); return; }
    let listening = true;
    const finish = () => { if (listening) settle(); };
    void reveal.finished.then(finish, finish);
    return () => { listening = false; };
  }, [animation]);

  const Drawing = drawings[palette];
  return <svg className="ccc-preset-artwork" data-preset-palette={palette} data-switching={animation.playing || undefined}
    width="480" height="204" viewBox="0 0 480 204" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={`${id}-body`} x1="0%" y1="12%" x2="94%" y2="82%">
        <stop stopColor="var(--ccc-preset-light)" />
        <stop offset=".48" stopColor="var(--ccc-preset-mid)" />
        <stop offset="1" stopColor="var(--ccc-preset-low)" />
      </linearGradient>
      <linearGradient id={`${id}-edge`}>
        <stop stopColor="var(--ccc-preset-light)" stopOpacity="0" />
        <stop offset=".72" stopColor="var(--ccc-preset-light)" />
        <stop offset="1" stopColor="var(--ccc-preset-light)" stopOpacity=".65" />
      </linearGradient>
      <radialGradient id={`${id}-haze`}>
        <stop stopColor="var(--ccc-preset-mid)" stopOpacity=".23" />
        <stop offset="1" stopColor="var(--ccc-preset-mid)" stopOpacity="0" />
      </radialGradient>
    </defs>
    <ellipse cx="240" cy="111" rx="218" ry="88" fill={`url(#${id}-haze)`} />
    <g className="ccc-preset-drawing" key={mode} ref={drawing}>
      <Drawing body={`url(#${id}-body)`} edge={`url(#${id}-edge)`} />
    </g>
  </svg>;
}
