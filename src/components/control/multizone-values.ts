import type { MultiZoneValues } from "../../api/client";
import { hexOf } from "../../lib/color";

export const MULTI_ZONE_MODES = [
  { value: "0", label: "随机" }, { value: "1", label: "静态分区" }, { value: "2", label: "呼吸" }, { value: "3", label: "循环" },
  { value: "4", label: "波浪" }, { value: "5", label: "舞动" }, { value: "6", label: "节奏" }, { value: "7", label: "闪烁" },
];
export function multiZoneName(mode: number) { return MULTI_ZONE_MODES.find(item => item.value === String(mode))?.label ?? "未知"; }
export function multiZoneSummary(values: MultiZoneValues, showPowerState = true) {
  const state = showPowerState ? values.enabled ? "开启 · " : "关闭 · " : "";
  return `${state}${multiZoneName(values.mode)} · 亮度${values.brightness}级 · 速度${values.speed}级`;
}
export function multiZoneChanges(before: MultiZoneValues, after: MultiZoneValues) {
  const changes: string[] = [];
  if (before.enabled !== after.enabled) changes.push(after.enabled ? "开启键盘灯" : "关闭键盘灯");
  if (before.mode !== after.mode) changes.push(`模式：${multiZoneName(before.mode)} → ${multiZoneName(after.mode)}`);
  if (before.brightness !== after.brightness) changes.push(`亮度：${before.brightness} → ${after.brightness}级`);
  if (before.speed !== after.speed) changes.push(`速度：${before.speed} → ${after.speed}级`);
  const colors = [...before.zones.map((value, index) => ({ name: `第${index + 1}区`, before: value, after: after.zones[index] })),
    { name: "灯条", before: before.lightBar, after: after.lightBar }, { name: "顶盖", before: before.cover, after: after.cover }];
  for (const color of colors) if (color.before && color.after && hexOf(color.before) !== hexOf(color.after))
    changes.push(`${color.name}：${hexOf(color.before)} → ${hexOf(color.after)}`);
  return changes.join("；");
}
