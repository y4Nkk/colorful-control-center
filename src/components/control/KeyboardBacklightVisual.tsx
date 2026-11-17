export function KeyboardBacklightVisual({ level, maximum, enabled, label }: { level: number | null; maximum: number; enabled: boolean | null; label: string }) {
  const valid = level !== null && Number.isInteger(level) && level >= 0 && level <= maximum;
  const brightness = valid ? level / maximum : 0;
  const value = !valid || enabled === null ? "未读取" : enabled ? `${level} 级` : `已关闭${level > 0 ? ` · 保留 ${level} 级` : ""}`;
  const lit = valid && enabled === true && level > 0;
  return <div className="ccc-backlight-visual" role="img" aria-label={`${label}：${value}`}>
    <span className="ccc-control-caption">{label}</span>
    <svg viewBox="0 0 320 118" aria-hidden="true" data-backlight-level={valid ? level : "unknown"} data-backlight-enabled={enabled ?? "unknown"}>
      <rect x="2" y="2" width="316" height="108" rx="12" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
      {Array.from({ length: 4 }, (_, row) => Array.from({ length: 14 }, (_, column) => <rect key={`${row}-${column}`}
        x={10 + column * 21.5} y={10 + row * 19} width="19" height="15" rx="3" fill="var(--ccc-content)"
        stroke="currentColor" strokeOpacity={lit ? .15 + brightness * .7 : .08} />))}
      <rect x="77" y="88" width="166" height="14" rx="3" fill="var(--ccc-content)" stroke="currentColor" strokeOpacity={lit ? .15 + brightness * .7 : .08} />
    </svg>
    <strong className="ccc-mono">{value === "未读取" ? "—" : value}</strong>
  </div>;
}
