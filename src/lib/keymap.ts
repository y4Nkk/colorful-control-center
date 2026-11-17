import keys from "./keymap-keys.json";
import type { KeymapAction, KeymapConfiguration, KeymapDevice, KeymapProfile, KeymapStep } from "../api/keymap";

export const KEYMAP_DEVICE_LABELS = { keyboard: "键盘", mouse: "鼠标" };
export const KEY_OPTIONS = keys.map(key => ({ value: String(key.scanCode), label: key.label }));
export const MOUSE_OPTIONS = [{ value: "1", label: "左键" }, { value: "4", label: "右键" }, { value: "16", label: "中键" },
  { value: "64", label: "后退键" }, { value: "256", label: "前进键" }];
export const keyLabel = (device: KeymapDevice, code: number) => (device === "keyboard" ? KEY_OPTIONS : MOUSE_OPTIONS)
  .find(key => key.value === String(code))?.label ?? `键码 0x${code.toString(16).padStart(4,"0").toUpperCase()}`;
export function optionsFor(code: number) {
  return KEY_OPTIONS.some(key => key.value === String(code)) ? KEY_OPTIONS : [{ value: String(code), label: keyLabel("keyboard", code) }, ...KEY_OPTIONS];
}
export function actionLabel(action: KeymapAction) {
  switch (action.kind) {
    case "disable": return "禁用按键";
    case "keys": return `按键宏 · ${action.steps.length} 步`;
    case "text": return `文本宏 · ${Array.from(action.text).length} 字`;
    case "launch": return `启动 ${action.path.split(/[\\/]/).pop()}`;
  }
}
export function actionDetails(action: KeymapAction): string {
  const click = (key: { scanCode: number; makeDelayMs: number; breakDelayMs: number }) =>
    `${keyLabel("keyboard", key.scanCode)}（按下后${key.makeDelayMs}ms，释放后${key.breakDelayMs}ms）`;
  switch (action.kind) {
    case "disable": return "禁用按键";
    case "keys": return action.steps.map((step, index) => `${index + 1}. ${step.kind === "key"
      ? `${step.pressed ? "按下" : "释放"} ${keyLabel("keyboard", step.scanCode)}，随后${step.delayAfterMs}ms`
      : `点击${keyLabel("mouse", step.button)}，按下后${step.makeDelayMs}ms，释放后${step.breakDelayMs}ms`}`).join("\n");
    case "text": return `前置：${action.before.map(click).join("、") || "无"}\n文本：${action.text}\n后置：${action.after.map(click).join("、") || "无"}`;
    case "launch": return `启动程序：${action.path}`;
  }
}
export function stepError(steps: KeymapStep[]): string | null {
  if (!steps.length || steps.length > 256) return "按键宏须包含1–256个步骤。";
  const held = new Set<number>();
  for (const step of steps) {
    if (step.kind === "mouseClick") continue;
    if (step.pressed) held.add(step.scanCode);
    else if (!held.delete(step.scanCode)) return "存在尚未按下的释放动作，请调整步骤。";
  }
  return held.size ? "宏结束前须释放所有按键。" : null;
}
export function configurationError(config: KeymapConfiguration): string | null {
  for (const device of config.devices) {
    for (const binding of device.bindings) {
      const action = binding.action;
      const error = action.kind === "keys" ? stepError(action.steps) : action.kind === "text" && (!action.text || action.text.includes("\0")) ? "文本宏不能为空或包含空字符。"
        : action.kind === "launch" && (!action.path.trim() || action.path.includes("\0")) ? "请选择程序完整路径。" : null;
      if (error) return `${keyLabel(device.device, binding.trigger)}：${error}`;
    }
  }
  return null;
}
export function changedDevices(profile: KeymapProfile, draft: KeymapConfiguration): KeymapConfiguration {
  return { devices: draft.devices.filter(device => JSON.stringify(device) !== JSON.stringify(profile.configuration?.devices.find(before => before.device === device.device))
    || profile.enabledFlags[device.device === "keyboard" ? 0 : 1] !== Number(device.enabled)) };
}
export function keymapChanges(profile: KeymapProfile, draft: KeymapConfiguration) {
  return changedDevices(profile, draft).devices.flatMap(device => {
    const before = profile.configuration?.devices.find(before => before.device === device.device);
    const label = KEYMAP_DEVICE_LABELS[device.device], items: { label: string; from: string; to: string }[] = [];
    const enabled = profile.enabledFlags[device.device === "keyboard" ? 0 : 1] === 1;
    if (enabled !== device.enabled) items.push({ label: `${label}启用`, from: enabled ? "开启" : "关闭", to: device.enabled ? "开启" : "关闭" });
    for (const code of new Set([...(before?.bindings.map(binding => binding.trigger) ?? []), ...device.bindings.map(binding => binding.trigger)])) {
      const old = before?.bindings.find(binding => binding.trigger === code), next = device.bindings.find(binding => binding.trigger === code);
      if (JSON.stringify(old) !== JSON.stringify(next)) items.push({ label: `${label} · ${keyLabel(device.device, code)}`,
        from: old ? actionDetails(old.action) : "原键", to: next ? actionDetails(next.action) : "恢复原键" });
    }
    return items;
  });
}
