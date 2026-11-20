import { applyProfileMode } from "./mock-profile-mode.mjs";

export function captureProfileGpuPower(data) {
  const devices = data.gpuPower.devices;
  if (!devices.length) return null;
  const device = devices[0];
  if (devices.length !== 1 || device.problem !== (device.enabled ? 0 : 22)) throw new Error("独显节点状态无法捕获");
  return { id: device.id, name: device.name, values: { enabled: device.enabled, automaticPowerOff: device.automaticPowerOff } };
}
export function previewProfileGpuPower(data, settings) {
  if (!settings.gpuPower) return null;
  if (data.profileGpuPreviewFault) throw new Error(data.profileGpuPreviewFault);
  const { id, values } = settings.gpuPower;
  const before = data.gpuPower.devices.find(device => device.id === id);
  if (!before || before.problem !== (before.enabled ? 0 : 22)) throw new Error("独显实例不存在或存在故障");
  const changes = before.enabled !== values.enabled || before.automaticPowerOff !== values.automaticPowerOff;
  if (changes && !values.enabled && !before.canDisable) throw new Error("独显输出在使用中，不能关断");
  if (!values.enabled && (settings.gpu || settings.fanSoftwareCurves)) throw new Error("独显关闭目标不能配置GPU偏移或软件曲线");
  return { before: structuredClone(before), target: structuredClone(values), softwareFanRunning: data.fan.software !== null };
}
export function applyProfileGpuPower(data, settings) {
  const review = previewProfileGpuPower(data, settings);
  if (!review) return;
  const current = data.gpuPower.devices.find(device => device.id === review.before.id);
  if (review.before.enabled !== review.target.enabled || review.before.automaticPowerOff !== review.target.automaticPowerOff) {
    data.profileGpuAuthorizations = (data.profileGpuAuthorizations ?? 0) + 1;
    if (data.gpuPowerFault) throw new Error(data.gpuPowerFault);
    data.gpuPowerInitial ??= structuredClone(current);
    Object.assign(current, review.target, { problem: review.target.enabled ? 0 : 22 });
    data.gpuPower.canRestore = true;
  }
  if (!review.target.enabled) data.fan.software = null;
  data.gpu.available = review.target.enabled && data.profileGpuDriverReady !== false;
  if (settings.gpu) {
    if (!data.gpu.available) throw new Error("开启后GPU驱动复核失败");
    if (settings.gpu.coreOffsetMhz !== null) data.gpu.coreOffsetMhz = settings.gpu.coreOffsetMhz;
    if (settings.gpu.memOffsetMhz !== null) data.gpu.memOffsetMhz = settings.gpu.memOffsetMhz;
    if (data.modeGpuCurrent) {
      if (settings.gpu.coreOffsetMhz !== null) data.modeGpuCurrent.offsets[0].coreKhz = settings.gpu.coreOffsetMhz * 1000;
      if (settings.gpu.memOffsetMhz !== null) data.modeGpuCurrent.offsets[0].memKhz = settings.gpu.memOffsetMhz * 1000;
    }
  }
}
export async function commitProfileTargets(data, settings) {
  const before = structuredClone({ devices: data.gpuPower.devices, gpu: data.gpu, software: data.fan.software, graphicsMode: data.graphicsMode, multiZone: data.multiZone, lightingOptions: data.lightingOptions, microphones: data.microphones ?? invokeMicrophones(data, "read_microphones", {}), touchpad: data.touchpad ?? invokeTouchpad(data, "read_touchpad", {}) });
  let multiZoneApplied = false, optionsApplied = false, microphonesApplied = false, touchpadApplied = false;
  const modeBefore = structuredClone({ energy: data.modeEnergy, ac: data.acMode, options: data.performanceOptions, clocks: data.modeGpuCurrent });
  let modeApplied = false;
  try {
    if (settings.oemPowerMode !== null) {
      const expected = await previewProfileMode(data, settings);
      applyProfileMode(data, expected);
      modeApplied = true;
      if (settings.performanceOptions) data.performanceOptions.current = structuredClone(settings.performanceOptions);
    }
    microphonesApplied = applyProfileMicrophones(data, settings);
    touchpadApplied = applyProfileTouchpad(data, settings);
    multiZoneApplied = applyProfileMultiZone(data, settings);
    optionsApplied = applyProfileLightingOptions(data, settings);
    applyProfileGpuPower(data, settings);
    applyProfileGraphicsMode(data, settings);
    data.profileApplication.steps = profileApplicationSteps(data, settings);
    data.profileApplication.targetsApplied = data.profileTargetsApplied ?? data.profileApplication.steps.every(step => step.ok);
    if (data.profileApplication.targetsApplied && settings.fanSoftwareCurves) data.fan.software = structuredClone(settings.fanSoftwareCurves);
  } catch (error) {
    data.profileApplication.steps = [{ step: "设备档案执行", ok: false, message: String(error) }];
    data.profileApplication.targetsApplied = false;
  }
  if (!data.profileApplication.targetsApplied && multiZoneApplied) {
    data.multiZone.current = before.multiZone.current;
    data.multiZone.revision = `multi-${++data.multiZoneRevision}`;
    data.profileApplication.steps.push({ step: "恢复多区键盘灯效", ok: true, message: "已恢复本次前态" });
  }
  if (!data.profileApplication.targetsApplied && optionsApplied) {
    data.lightingOptions.current = before.lightingOptions.current;
    data.lightingOptions.revision = `lighting-options-${++data.lightingOptionsRevision}`;
    data.profileApplication.steps.push({ step: "恢复灯光策略与外围灯区", ok: true, message: "已恢复本次前态" });
  }
  if (!data.profileApplication.targetsApplied && touchpadApplied) {
    data.touchpad.current = before.touchpad.current;
    data.touchpad.revision = "touchpad-" + (data.touchpadRevision = (data.touchpadRevision ?? 0) + 1);
    data.profileApplication.steps.push({ step: "恢复本次触摸板状态", ok: true, message: "已恢复本次前态" });
  }
  if (!data.profileApplication.targetsApplied && microphonesApplied) {
    data.microphones.current = before.microphones.current;
    data.microphones.revision = "microphone-" + (data.microphoneRevision = (data.microphoneRevision ?? 0) + 1);
    data.profileApplication.steps.push({ step: "恢复本次麦克风状态", ok: true, message: "已恢复本次前态" });
  }
  if (!data.profileApplication.targetsApplied) restoreProfileGraphicsMode(data, before.graphicsMode);
  if (!data.profileApplication.targetsApplied && (settings.gpuPower || modeApplied)) {
    data.gpuPower.devices = before.devices; data.gpu = before.gpu; data.fan.software = before.software;
    data.profileApplication.steps.push({ step: "恢复本次独显状态", ok: true, message: "已恢复本次前态" });
  }
  if (!data.profileApplication.targetsApplied && modeApplied) {
    data.modeEnergy = modeBefore.energy; data.acMode = modeBefore.ac;
    data.modeGpuCurrent = modeBefore.clocks;
    Object.assign(data.performanceOptions, modeBefore.options, { revision: `options-${++data.performanceRevision}` });
    data.profileApplication.steps.push({ step: "恢复本次模式与固件节能", ok: true, message: "已恢复本次前态" });
  }
}
