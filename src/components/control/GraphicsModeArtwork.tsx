type GraphicsRoute = "integrated" | "discrete" | "hybrid" | "dynamic";

/** 选中项里位于通路上的 GPU 用实心挖空字形，未选中用线框；不在通路上的 GPU 淡出。图形只表达选项含义。 */
export function GraphicsModeArtwork({ route, selected }: { route: GraphicsRoute; selected: boolean }) {
  const integrated = route !== "discrete";
  const discrete = route !== "integrated";
  const dynamic = route === "dynamic";
  const junction = dynamic ? 74.5 : 104.5;

  return <svg className="ccc-graphics-choice-art" viewBox="0 0 148 74" fill="none" stroke="currentColor"
    strokeWidth="2.25" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
    <g opacity={integrated ? 1 : .3}>
      {selected && integrated
        ? <path data-solid fill="currentColor" fillRule="evenodd" stroke="none"
          d="M13.5 5.5h10A5.5 5.5 0 0 1 29 11v10a5.5 5.5 0 0 1-5.5 5.5h-10A5.5 5.5 0 0 1 8 21V11a5.5 5.5 0 0 1 5.5-5.5Zm3 6a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2Z" />
        : <>
          <rect x="8" y="5.5" width="21" height="21" rx="5.5" fill={integrated ? "currentColor" : "none"} fillOpacity=".1" />
          <rect x="14.5" y="11.5" width="8" height="8" rx="2" strokeOpacity=".62" />
        </>}
      <path d="M13.5 5.5V3M18.5 5.5V3M23.5 5.5V3M13.5 26.5V29M18.5 26.5V29M23.5 26.5V29M8 11H5.5M8 16H5.5M8 21H5.5M29 11h2.5M29 16h2.5M29 21h2.5" />
    </g>
    <g opacity={discrete ? 1 : .3}>
      {selected && discrete
        ? <path data-solid fill="currentColor" fillRule="evenodd" stroke="none"
          d="M9.5 50h18a3.5 3.5 0 0 1 3.5 3.5v9a3.5 3.5 0 0 1-3.5 3.5h-18A3.5 3.5 0 0 1 6 62.5v-9A3.5 3.5 0 0 1 9.5 50Zm4 4.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Zm10 0a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" />
        : <>
          <rect x="6" y="50" width="25" height="16" rx="3.5" fill={discrete ? "currentColor" : "none"} fillOpacity=".1" />
          <circle cx="13.5" cy="58" r="3.5" /><circle cx="23.5" cy="58" r="3.5" />
        </>}
      <path d="M3.5 47.5v21M14.5 66v2.5a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2V66" />
    </g>
    <rect x="112" y="26" width="34" height="22" rx="4" fill="currentColor" fillOpacity=".1" />
    <path d="M129 48v7M121.5 56.5h15" />
    <g data-routes strokeDasharray={dynamic ? ".1 5.5" : undefined}>
      {integrated && <path d={`M31.5 16H53a10.5 10.5 0 0 1 10.5 10.5a10.5 10.5 0 0 1 10.5 10.5H${junction}`} />}
      {route === "hybrid"
        ? <path d="M18.5 46.5V34.5" />
        : discrete && <path d={`M31 58H53a10.5 10.5 0 0 0 10.5-10.5a10.5 10.5 0 0 0 10.5-10.5H${junction}`} />}
      {dynamic && <path d="M94.5 37h10" />}
    </g>
    <path d="M104.5 33.5 108 37l-3.5 3.5" />
    {route === "hybrid" && <path d="M15 38 18.5 34.5 22 38" />}
    {dynamic && <g data-switch strokeWidth="2">
      <circle cx="84.5" cy="37" r="10" />
      <path d="M79.75 34h9.5m-3-3 3 3-3 3M89.25 40h-9.5m3-3-3 3 3 3" />
    </g>}
  </svg>;
}
