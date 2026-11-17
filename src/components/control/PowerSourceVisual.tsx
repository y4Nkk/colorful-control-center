export type PowerSource = "ac" | "battery" | "ups" | "unknown" | "adapter";
const NAMES = { ac: "已插电", battery: "电池供电", ups: "备用电源", unknown: "供电未知", adapter: "PD 适配器" };
export function PowerSourceVisual({ source, watts, label }: { source: PowerSource; watts?: number | null; label?: string }) {
  return <div className="ccc-power-source-visual" role="img" aria-label={`${label ?? NAMES[source]}${watts != null ? ` · ${watts} W` : source === "adapter" ? " · 未报告功率" : ""}`}>
    <svg viewBox="0 0 120 100" aria-hidden="true" data-power-source={source}>
      {source === "ac" || source === "adapter" ? <>
        <path d="M12 72H5m95-29h14v22H98" fill="none" stroke="var(--ccc-hairline)" strokeWidth="3" />
        <rect x="15" y="16" width="83" height="68" rx="15" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
        <path d="M59 28 42 53h15l-4 20 20-28H59Z" fill="currentColor" opacity=".75" />
        <path d="M31 22h48" stroke="var(--ccc-content)" strokeWidth="3" strokeLinecap="round" />
      </> : source === "battery" || source === "ups" ? <>
        <rect x="13" y="22" width="88" height="57" rx="12" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
        <path d="M103 40h6v20h-6" fill="var(--ccc-hairline)" />
        <path d="M31 51h15m-7-8v16m26-8h15" stroke="currentColor" strokeWidth="3" strokeLinecap="round" opacity=".6" />
      </> : <>
        <rect x="15" y="18" width="87" height="66" rx="15" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" strokeDasharray="4 5" />
        <path d="M51 41a10 10 0 1 1 15 9l-7 5v5m0 8v1" fill="none" stroke="var(--ccc-text-muted)" strokeWidth="3" strokeLinecap="round" />
      </>}
    </svg>
    <div><span className="ccc-control-caption">{label ?? "当前供电"}</span><strong>{watts != null ? `${watts} W` : source === "adapter" ? "未报告功率" : NAMES[source]}</strong></div>
  </div>;
}
