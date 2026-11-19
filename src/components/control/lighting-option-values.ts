import type { KeyboardZone, LightingOptionsValues, TopCoverValues } from "../../api/client";
import { hexOf } from "../../lib/color";
export const booleanText = (value: boolean) => value ? "开启" : "关闭";
export const timerText = (value: LightingOptionsValues["idleTimer"]) => value.enabled ? `${value.seconds} 秒后关灯` : `不自动关灯（预设 ${value.seconds} 秒）`;
export const zoneText = (value: KeyboardZone) => `${value.enabled ? "开启" : "关闭"} · ${hexOf(value.color).toUpperCase()}`;
export const topCoverText = (value: TopCoverValues) => `${value.brightness === 0 ? "关闭" : `亮度${value.brightness}级`} · ${hexOf(value.color).toUpperCase()}`;

export const IDLE_TIME_FIELDS = [
  { key: "hours", label: "时", name: "闲置等待小时", max: 1 },
  { key: "minutes", label: "分", name: "闲置等待分钟", max: 59 },
  { key: "seconds", label: "秒", name: "闲置等待秒数", max: 59 },
] as const;

export type IdleTimerDraft = { enabled: boolean } & Record<typeof IDLE_TIME_FIELDS[number]["key"], number | "">;
export type LightingOptionsDraft = Omit<LightingOptionsValues, "idleTimer"> & { idleTimer: IdleTimerDraft };

export function idleTimerDraft({ enabled, seconds }: LightingOptionsValues["idleTimer"]): IdleTimerDraft {
  return { enabled, hours: Math.floor(seconds / 3600), minutes: Math.floor(seconds % 3600 / 60), seconds: seconds % 60 };
}

export function idleTimeFieldValid(value: number | "", max: number) {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= max;
}

export function idleTimerSeconds(draft: IdleTimerDraft): number | null {
  if (IDLE_TIME_FIELDS.some(({ key, max }) => !idleTimeFieldValid(draft[key], max))) return null;
  return Number(draft.hours) * 3600 + Number(draft.minutes) * 60 + Number(draft.seconds);
}

export function lightingOptionChanges(current: LightingOptionsValues, target: LightingOptionsValues) {
  const facts = [{ label: "闲置关灯", before: timerText(current.idleTimer), after: timerText(target.idleTimer) }];
  if (current.bootOverride !== null && target.bootOverride !== null) {
    facts.push({ label: "开机灯效覆盖", before: booleanText(current.bootOverride), after: booleanText(target.bootOverride) });
  }
  if (current.logo && target.logo) facts.push({ label: "Logo 灯", before: zoneText(current.logo), after: zoneText(target.logo) });
  if (current.lightBar && target.lightBar) facts.push({ label: "灯条", before: zoneText(current.lightBar), after: zoneText(target.lightBar) });
  if (current.topCover && target.topCover) facts.push({ label: "独立顶盖", before: topCoverText(current.topCover), after: topCoverText(target.topCover) });
  return facts.filter(({ before, after }) => before !== after).map(({ label, before, after }) => ({ label, value: `${before} → ${after}` }));
}

