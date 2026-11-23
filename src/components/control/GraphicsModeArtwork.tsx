type GraphicsRoute = "integrated" | "discrete" | "hybrid" | "dynamic";

export function GraphicsModeArtwork({ route }: { route: GraphicsRoute }) {
  const integrated = route !== "discrete";
  const discrete = route !== "integrated";
  const dynamic = route === "dynamic";

  return <svg className="ccc-graphics-choice-art" viewBox="0 0 158 82" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
    <g opacity={integrated ? 1 : .3}>
      <rect x="14" y="11" width="28" height="24" rx="5" />
      <rect x="22" y="17" width="12" height="12" rx="2" />
      <path d="M10 18h4m-4 10h4m28-10h4m-4 10h4" />
    </g>
    <g opacity={discrete ? 1 : .3}>
      <rect x="13" y="47" width="30" height="24" rx="4" />
      <circle cx="22" cy="59" r="3.5" />
      <circle cx="34" cy="59" r="3.5" />
      <path d="M9 45v28m8-2v3h20v-3" />
    </g>
    <g strokeDasharray={dynamic ? "3 4" : undefined}>
      {integrated && <path d="M48 23h12q6 0 6 6v6q0 6 6 6h8" />}
      {discrete && <path d="M48 59h12q6 0 6-6v-6q0-6 6-6h8" />}
    </g>
    {dynamic ? <path d="m84 37 4 4-4 4-4-4z" /> : <circle cx="84" cy="41" r="4" />}
    <path d="M88 41h19" />
    <rect x="107" y="26" width="41" height="29" rx="4" />
    <path d="M113 49h29m-14.5 6v7m-8 0h16" />
  </svg>;
}
