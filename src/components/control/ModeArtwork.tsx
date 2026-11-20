export const MODE_ARTWORK_BY_MODE = ["quiet", "eco", "performance", "entertainment", "quiet"] as const;

function Eco() {
  return <>
    <path d="M8.5 14h28a3.5 3.5 0 0 1 3.5 3.5V20h2a1.5 1.5 0 0 1 1.5 1.5v5A1.5 1.5 0 0 1 42 28h-2v2.5a3.5 3.5 0 0 1-3.5 3.5h-28A3.5 3.5 0 0 1 5 30.5v-13A3.5 3.5 0 0 1 8.5 14Z" />
    <path d="M35.5 21v6" strokeWidth="1.5" strokeOpacity=".58" />
    <g strokeWidth="1.5">
      <path d="M14.7 28.5c.6-6.2 5.9-9.9 15.1-9.8-.6 7.5-4.3 11.3-10 11.3-1.9 0-3.6-.6-5.1-1.5Z"
        fill="var(--ccc-mode-art-tint)" />
      <path d="M13.4 31c2.8-4 6.3-6.7 10.9-8.4" />
    </g>
  </>;
}

function Quiet() {
  return <>
    <path d="M28.8 7.2C19.1 4.7 9.5 10.1 6.9 19.8c-2.5 9.8 3.4 19.7 13 22 8.8 2.1 17.4-2.4 20.9-10.3-7.2 1.5-14-2.1-16.1-8.6-1.8-5.6-.1-11.5 4.1-15.7Z"
      fill="var(--ccc-mode-art-tint)" />
    <path d="M13.4 18.1c-3.5 7.2-.5 14.9 6.1 18.2" stroke="var(--ccc-mode-art-mark)" strokeWidth="1.5" opacity=".56" />
  </>;
}

function Entertainment() {
  return <>
    <path d="M15 12.5h18c4.8 0 7.8 2.7 8.8 7.1l2.2 10.1c1.4 6.3-4.2 8.7-7.5 5.1l-4.7-5.1H16.2l-4.7 5.1C8.2 38.4 2.6 36 4 29.7l2.2-10.1c1-4.4 4-7.1 8.8-7.1Z" />
    <path d="M11.2 21.5h6.6m-3.3-3.3v6.6M22.5 17.7h3" strokeWidth="1.5" />
    <circle cx="33.5" cy="21.5" r="6.8" fill="var(--ccc-mode-art-tint)" stroke="none" />
    <g fill="var(--ccc-mode-art-ink)" stroke="none">
      <circle cx="34.2" cy="18.7" r="1.6" />
      <circle cx="37.3" cy="22.9" r="1.6" />
    </g>
  </>;
}

function Performance() {
  return <>
    <path d="M9 33a16.8 16.8 0 1 1 30 0" />
    <path d="M12.5 25.5H15m1.2-8.1 1.8 1.8M24 14v2.5m6 2.7 1.8-1.8m1.2 8.1h2.5M13 37h22"
      strokeWidth="1.5" />
    <g transform="translate(24 29)">
      <path d="M-1.5 0 5.9-10.2c.7-.9 2 .1 1.2 1L1.5 1Z" fill="var(--ccc-mode-art-tint)" strokeWidth="1.5" />
      <circle r="2.2" fill="var(--ccc-content)" strokeWidth="1.5" />
    </g>
  </>;
}

const DRAWINGS = { eco: Eco, quiet: Quiet, entertainment: Entertainment, performance: Performance };

export function ModeArtwork({ mode }: { mode: number }) {
  const artwork = MODE_ARTWORK_BY_MODE[mode];
  if (!artwork) return null;
  const Drawing = DRAWINGS[artwork];
  return <svg className="ccc-mode-art" data-mode-art={artwork} width="48" height="48" viewBox="0 0 48 48"
    fill="none" stroke="var(--ccc-mode-art-ink)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
    aria-hidden="true" focusable="false">
    <Drawing />
  </svg>;
}
