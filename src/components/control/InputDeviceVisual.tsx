/** The glyph depicts the control. The adjacent text carries the reported state. */
export function InputDeviceVisual({ kind }: { kind: "touchpad" | "microphone" }) {
  return <svg className="ccc-device-art" viewBox="0 0 96 64" aria-hidden="true">
    <ellipse cx="48" cy="56" rx="34" ry="4" fill="currentColor" opacity=".09" />
    {kind === "touchpad" ? <>
      <path d="M21 17h54l12 29a5 5 0 0 1-5 7H14a5 5 0 0 1-5-7Z"
        fill="var(--ccc-content-muted)" stroke="currentColor" strokeOpacity=".24" strokeWidth="1.5" />
      <path d="M29 23h38l8 21H21Z" fill="currentColor" opacity=".1" />
      <ellipse cx="48" cy="32" rx="13" ry="6" fill="none" stroke="currentColor" strokeOpacity=".46" strokeWidth="1.5" />
      <ellipse cx="48" cy="32" rx="5" ry="2.5" fill="currentColor" opacity=".72" />
    </> : <>
      <rect x="39" y="8" width="18" height="32" rx="9" fill="currentColor" opacity=".24" />
      <path d="M39 17a9 9 0 0 1 18 0v14a9 9 0 1 1-18 0ZM32 32a16 16 0 0 0 32 0M48 48v8m-9 0h18"
        fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <path d="M44 18h8m-8 5h8m-8 5h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </>}
  </svg>;
}
