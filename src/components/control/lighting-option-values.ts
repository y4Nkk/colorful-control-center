import type { KeyboardZone, LightingOptionsValues, TopCoverValues } from "../../api/client";
import { hexOf } from "./useKeyboardLighting";
export const booleanText = (value: boolean) => value ? "开启" : "关闭";
export const timerText = (value: LightingOptionsValues["idleTimer"]) => value.enabled ? `${value.seconds} 秒后关灯` : `不自动关灯（预设 ${value.seconds} 秒）`;
export const zoneText = (value: KeyboardZone) => `${value.enabled ? "开启" : "关闭"} · ${hexOf(value.color).toUpperCase()}`;
export const topCoverText = (value: TopCoverValues) => `${value.brightness === 0 ? "关闭" : `亮度${value.brightness}级`} · ${hexOf(value.color).toUpperCase()}`;

