import type { EnergyPolicy } from "../../api/battery";
import { energyHour } from "../../lib/energy-policy";

export function EnergyTimeline({ slots, label }: { slots: EnergyPolicy["slots"]; label: string }) {
  const summary = slots.flatMap(slot => slot ? [`${energyHour(slot.start)}到${energyHour(slot.end)}`] : []).join("，") || "无";
  return <div>
    <svg className="ccc-energy-timeline" viewBox="0 0 600 36" preserveAspectRatio="none" role="img" aria-label={`${label}：${summary}`}>
      <rect x="0" y="6" width="600" height="24" rx="8" fill="var(--ccc-chart-track)" />
      {slots.map((slot, index) => slot && slot.start < slot.end && <rect key={index} data-energy-slot={index}
        x={slot.start / 24 * 600} y="6" width={(slot.end - slot.start) / 24 * 600} height="24" rx="7" fill="var(--ccc-chart-teal)" />)}
      {[6, 12, 18].map(hour => <path key={hour} d={`M${hour * 25} 13v10`} stroke="var(--ccc-chart-label)" opacity=".5" />)}
    </svg>
    <div className="ccc-energy-hours ccc-mono" aria-hidden="true">{[0, 6, 12, 18, 24].map(hour => <span key={hour}>{hour}</span>)}</div>
  </div>;
}
