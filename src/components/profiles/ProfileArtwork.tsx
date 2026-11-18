export function ProfileArtwork() {
  return <svg className="ccc-profile-art" viewBox="0 0 240 150" aria-hidden="true">
    <ellipse cx="120" cy="130" rx="84" ry="10" fill="currentColor" opacity=".07" />
    {[70, 51, 32].map((y, index) => <g key={y} transform={`translate(0 ${y})`}>
      <path d="M46 28 113 3a20 20 0 0 1 14 0l67 25a5 5 0 0 1 0 9l-67 25a20 20 0 0 1-14 0L46 37a5 5 0 0 1 0-9Z"
        fill="var(--ccc-content)" stroke="currentColor" strokeOpacity=".36" strokeWidth="1.5" />
      <path d="M46 28 113 3a20 20 0 0 1 14 0l67 25a5 5 0 0 1 0 9l-67 25a20 20 0 0 1-14 0L46 37a5 5 0 0 1 0-9Z"
        fill="currentColor" opacity={index === 2 ? .15 : .06} />
    </g>)}
    <path d="m90 59 37-14m-25 23 37-14m-25 23 37-14" stroke="currentColor" opacity=".64" strokeWidth="2.5" strokeLinecap="round" />
    <circle cx="117" cy="49" r="4.5" fill="currentColor" /><circle cx="114" cy="63" r="4.5" fill="currentColor" />
    <circle cx="139" cy="67" r="4.5" fill="currentColor" />
  </svg>;
}
