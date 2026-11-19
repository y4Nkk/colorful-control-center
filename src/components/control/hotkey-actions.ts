import type { IconName } from "../Icon";
import { NOISELESS_FAN_LABEL, type HotkeyScope, type HotkeySettingsValue } from "../../api/client";

export const ACTIONS: { scope: HotkeyScope; icon: IconName; title: string; rule: string }[] = [
  { scope: "powerMode", icon: "performance", title: "电源模式循环", rule: "按本机候选循环，执行已预览的独显开启或关断、GPU调频、固件节能及已保存的风扇与系统联动，并将所选模式记为插电偏好；硬件管理时跳过固件节能命令。" },
  { scope: "fan", icon: "fan", title: "风扇模式", rule: "按固件能力切换自动、全速、能效优先或自定义。自定义使用当前保存的曲线。" },
  { scope: "keyboardBrightness", icon: "lighting", title: "键盘亮度", rule: "逐级调节并开启灯光，保留颜色和灯效；最低为 1 级。" },
  { scope: "keyboardSwitch", icon: "lighting", title: "键盘灯开关", rule: "切换键盘灯总开关，保留亮度、颜色和灯效；按本机键盘家族执行。" },
  { scope: "keyboardZones", icon: "keyboard", title: "键盘分区开关", rule: "分别切换左、中、右区，保留总开关、亮度和颜色。" },
  { scope: "keyboardColor", icon: "lighting", title: "键盘颜色循环", rule: "按原厂 15／16 色顺序切换全键盘色，保留开关、亮度与灯效设置。" },
  { scope: "microphoneMute", icon: "microphone", title: "麦克风静音", rule: "全部静音时取消静音；混合状态时统一静音。只操作确认接管时的活动麦克风集合。" },
  { scope: "touchpad", icon: "touchpad", title: "触摸板开关", rule: "切换已确认设备的系统开关并保存偏好；状态未知或设备变化时停止执行。" },
  { scope: "fnLock", icon: "keyboard", title: "Fn 锁切换", rule: "只切换 Fn 锁，保留 Win 锁与按键交换。" },
  { scope: "mappings", icon: "keyboard", title: "映射开关", rule: "只切换本应用已经接管的设备；映射内容变化后需要重新确认。" },
  { scope: "openConsole", icon: "window", title: "唤起主窗口", rule: "显示主窗口。" },
  { scope: "stateNotifications", icon: "info", title: "原厂状态通知", rule: `只报告锁键、扩展坞、GPS、${NOISELESS_FAN_LABEL}、移动网络、除尘、Turbo 风扇与充电状态。按原厂规则识别到 USB 键盘时跳过数字锁提示。` },
  { scope: "keyboardBrightnessEvent", icon: "lighting", title: "原厂亮度循环与反馈", rule: "原厂支持的 RGB 键盘循环关闭与 1–4 档，关闭时保留亮度；白光读取固件档位并同步保存，不发送亮度命令。" },
  { scope: "cameraReports", icon: "info", title: "摄像头状态反馈", rule: "记录摄像头及摄像头／麦克风组合报告，按原厂规则同步摄像头保存值；不执行设备开关。恢复只还原初始软件策略。" },
  { scope: "screenBrightness", icon: "display", title: "原厂屏幕亮度策略", rule: "仅电池供电且当前更亮时按原厂触发目标降低亮度，同步修改活动计划的插电和电池亮度保存值。实际报告与计划值分开展示；初始亮度单独恢复。" },
  { scope: "panelHotkey", icon: "display", title: "原厂屏幕按键", rule: "按已确认面板和原厂能力选择空白屏保或一次性输入唤醒；独立授权，只结束本软件会话。" },
  { scope: "gpuTurbo", icon: "gpu", title: "原厂 GPU Turbo", rule: "按原厂事件、模式和供电切换 GPU 锁频及偏移；只对已确认显卡执行，并支持初始恢复。" },
  { scope: "batteryThresholds", icon: "battery", title: "原厂低电量阈值", rule: "按原厂事件设置各计划的低、严重和预留电量阈值；初始插电及电池值分别保留并可恢复。" },
  { scope: "powerDeliveryEvent", icon: "plug", title: "原厂 PD 档位事件", rule: "按原厂事件设置已确认适配器的固定档位；与平台电源共用初始恢复。" },
  { scope: "batteryColdWarning", icon: "temperature", title: "原厂电池低温提醒", rule: "原厂事件，以及电池供电时的启动和唤醒会检查温度；低于 11°C 且电量低于 81% 时提醒接入电源。" },
  { scope: "batteryLowBrightness", icon: "display", title: "原厂低电量亮度", rule: "原厂低电量事件按缓存状态调暗；结束时采用当前插电亮度或缓存值。首次插电和电池设置可另行确认恢复。" },
  { scope: "batteryLowAction", icon: "sleep", title: "原厂低电量系统动作", rule: "低电量事件可按当前电源计划请求睡眠或休眠；接电时不请求。此动作不会自动撤销，软件策略可单独恢复。" },
  { scope: "openKeyboard", icon: "keyboard", title: "打开键盘灯区域", rule: "进入「键盘与灯光」并定位到灯光控件。" },
  { scope: "displayRotation", icon: "display", title: "原厂屏幕旋转", rule: "定位主屏方向设置。切换后试用，15 秒内可保留。" },
  { scope: "batteryAlert", icon: "volume", title: "原厂电池音频事件", rule: "按原厂开始与结束事件控制静音循环及PC Beep控件；事件可指定屏幕绝对亮度，并按电池条件调暗。停止策略取消后续通知并恢复初始音频；亮度另行确认恢复。" },
  { scope: "systemRecovery", icon: "undo", title: "Windows 恢复入口", rule: "原厂按键打开 Windows 恢复选项，不自动执行还原或重置；恢复热键策略不关闭或撤销 Windows 操作。" },
  { scope: "systemShutdown", icon: "shutdown", title: "原厂关机请求", rule: "原厂事件请求系统关机；可先停止接管，策略恢复不撤销已发送的关机请求。" },
  { scope: "lidAction", icon: "sleep", title: "原厂合盖动作", rule: "按 Windows 当前计划的插电和电池设置执行，现代待机的睡眠目标改为关闭屏幕；恢复只还原接管策略。" },
  { scope: "airplaneMode", icon: "airplane", title: "飞行模式按键", rule: "切换系统飞行模式并同步原厂固件；首次状态可在飞行模式卡片中单独确认恢复。" },
  { scope: "panelEvent", icon: "display", title: "原厂面板维护", rule: "接管启动时修复适用的 NVSR 面板，原厂事件按面板身份发送 NVSR 修复或 OLED 唤醒。恢复只还原软件策略，不撤销一次性修复。" },
];
export const summary = (settings: HotkeySettingsValue) => settings.actions.length
  ? ACTIONS.filter(action => settings.actions.includes(action.scope)).map(action => action.title).join("、") : "尚未选择范围";

