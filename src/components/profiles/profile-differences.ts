import {
  ENERGY_POLICY_LABEL, OEM_POWER_MODES, fanModeLabel, offsetUnitsToMv,
  type DisplayMode, type ManualProfilePreview, type ProfileSettings,
} from "../../api/client";
import { formatParameter } from "../../lib/format-parameters";

export interface ProfileDifference {
  label: string;
  before: string | null;
  target: string;
  note?: string;
  detailsOnly?: boolean;
}

const mode = (value: number) => OEM_POWER_MODES[value] ?? "未知模式";
const graphicsMode = (value: number) => ({ 1: "集显", 2: "独显", 3: "混合", 4: "动态" })[value] ?? "未知模式";
const onOff = (enabled: boolean) => enabled ? "开启" : "关闭";
const readConfig = () => "已读取";
const savedConfig = () => "见全部设置";
const measure = (unit: string) => (value: number) => formatParameter(value, unit, 3);
const voltage = (value: number) => measure("mV")(offsetUnitsToMv(value));

export function sameProfileValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function entry<T>(label: string, before: T | null | undefined, target: T | null | undefined,
  format: (value: T) => string, note?: string, formatBefore = format): ProfileDifference[] {
  if (target == null || (before != null && sameProfileValue(before, target) && !note)) return [];
  const current = before == null ? null : formatBefore(before), next = format(target);
  return [{ label, before: current, target: next, detailsOnly: format === savedConfig,
    note: note ?? (current === next ? "部分参数有变化" : undefined) }];
}

function performance(s: ProfileSettings, b: ProfileSettings | null, review: ManualProfilePreview | null) {
  const firmware = review?.modeTarget ? "包含原厂模式联动" : undefined;
  const defaultConfig = <T>(label: string, before: T | null | undefined, target: T | null) =>
    entry(label, before, target, savedConfig, undefined, readConfig);
  return [
    ...entry("原厂模式", b?.oemPowerMode, s.oemPowerMode, mode, firmware),
    ...entry("风扇模式", b?.fanSoftwareCurves ? -1 : b?.fanMode, s.fanSoftwareCurves ? -1 : s.fanMode,
      value => value === -1 ? "软件曲线" : fanModeLabel(value)),
    ...defaultConfig("性能选项", b?.performanceOptions, s.performanceOptions),
    ...entry("持续功耗上限 · 保存值", b?.pl1W, s.pl1W, measure("W")),
    ...entry("短时功耗上限 · 保存值", b?.pl2W, s.pl2W, measure("W")),
    ...entry("核心电压偏移 · 保存值", b?.coreOffsetMv, s.coreOffsetMv, measure("mV")),
    ...entry("核显电压偏移 · 保存值", b?.gtOffsetMv, s.gtOffsetMv, measure("mV")),
    ...entry("持续功耗上限 · 即时", b?.runtimeCpu?.pl1W, s.runtimeCpu?.pl1W, measure("W")),
    ...entry("短时功耗上限 · 即时", b?.runtimeCpu?.pl2W, s.runtimeCpu?.pl2W, measure("W")),
    ...entry("核心电压偏移 · 即时", b?.runtimeCpu?.coreOffsetUnits, s.runtimeCpu?.coreOffsetUnits, voltage),
    ...entry("核显电压偏移 · 即时", b?.runtimeCpu?.gtOffsetUnits, s.runtimeCpu?.gtOffsetUnits, voltage),
    ...entry("核心频率偏移", b?.gpu?.coreOffsetMhz, s.gpu?.coreOffsetMhz, measure("MHz")),
    ...entry("显存频率偏移", b?.gpu?.memOffsetMhz, s.gpu?.memOffsetMhz, measure("MHz")),
  ];
}

function display(s: ProfileSettings, review: ManualProfilePreview | null) {
  const describe = (value: DisplayMode) => `${value.width} × ${value.height} · ${value.frequency} Hz`;
  const monitors = (s.display?.displays ?? []).flatMap(target => {
    const before = review?.displayReview?.displays.find(change => change.target.device === target.device)?.before;
    const different = before && !sameProfileValue(before, target);
    const format = before && before.orientation !== target.orientation
      ? (value: DisplayMode) => `${describe(value)} · ${value.orientation * 90}°` : describe;
    return entry(target.name, before, target, format, different ? "显示试用 15 秒，需保留确认" : undefined);
  });
  const graphics = review?.graphicsModeReview?.before;
  const pending = graphics?.lastRequest?.verification === "pendingReboot";
  const target = s.graphicsMode?.mode;
  const reboot = graphics && (pending || graphics.current?.mode !== target) ? "需重启生效" : undefined;
  const node = review?.gpuPowerReview;
  return [
    ...monitors,
    ...entry("显卡模式", graphics?.current?.mode, target, graphicsMode, reboot),
    ...entry("独显电源", node?.before.enabled, s.gpuPower?.values.enabled, onOff,
      node?.before.enabled && !s.gpuPower?.values.enabled && node.softwareFanRunning ? "将退出软件风扇曲线" : undefined),
    ...entry("独显自动关断", node?.before.automaticPowerOff, s.gpuPower?.values.automaticPowerOff, onOff),
  ];
}

function devices(s: ProfileSettings, b: ProfileSettings | null) {
  const settings = <T>(label: string, before: T | null | undefined, target: T | null) =>
    entry(label, before, target, savedConfig, undefined, readConfig);
  const charge = (value: NonNullable<ProfileSettings["batteryCharge"]>["values"]) =>
    value.enabled ? `${value.start}%–${value.stop}%` : "关闭";
  const lighting = (value: NonNullable<ProfileSettings["keyboardLighting"]>) =>
    `${onOff(value.enabled)} · 亮度 ${value.settings.brightness} 级`;
  return [
    ...entry("键盘灯光", b?.keyboardLighting, s.keyboardLighting, lighting),
    ...settings("多区灯光", b?.multiZoneLighting, s.multiZoneLighting),
    ...settings("灯光策略", b?.lightingOptions, s.lightingOptions),
    ...settings("逐键灯光", b?.perkeyLighting, s.perkeyLighting),
    ...settings("灯条", b?.lightBar, s.lightBar),
    ...entry("充电限制", b?.batteryCharge?.values, s.batteryCharge?.values, charge),
    ...settings(ENERGY_POLICY_LABEL, b?.batteryEnergy?.policy, s.batteryEnergy?.policy ?? null),
    ...entry("PD 供电", b?.powerDelivery?.level, s.powerDelivery?.level, value => `档位 ${value}`),
    ...settings("USB 供电", b?.usbPolicy, s.usbPolicy),
    ...settings("有线网卡", b?.lanPolicy, s.lanPolicy),
    ...settings("除尘计划", b?.dustSchedule, s.dustSchedule),
    ...entry("Fn 锁", b?.keyboard?.fnLocked, s.keyboard?.fnLocked, onOff),
    ...entry("Win 锁", b?.keyboard?.winLocked, s.keyboard?.winLocked, onOff),
    ...entry("Fn / Win 交换", b?.keyboard?.fnWinSwap, s.keyboard?.fnWinSwap, onOff),
    ...entry("触摸板", b?.touchpad?.enabled, s.touchpad?.enabled, onOff),
    ...settings("麦克风静音", b?.microphones, s.microphones),
    ...settings("键位与宏", b?.keymap, s.keymap),
  ];
}

export function profileDifferences(settings: ProfileSettings, review: ManualProfilePreview | null) {
  const before = review?.before ?? null;
  return [...performance(settings, before, review), ...display(settings, review), ...devices(settings, before)];
}
