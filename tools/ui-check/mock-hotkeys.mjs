export function gpuTurboFixture() {
  const id = { pciDevice: 0x10de1234, subsystem: 3, revision: 4, extended: 5, bus: 1, slot: 1 };
  const locks = [{ domain: 0, mode: 2, value: 1700000, unknown1: 0, unknown2: 0, unknown3: 0 },
    { domain: 1, mode: 0, value: 0, unknown1: 0, unknown2: 0, unknown3: 0 }];
  const offsets = { coreKhz: 100000, memKhz: -1000 };
  const saved = [{ coreMhz: 100, memoryMhz: -1 }, { coreMhz: 1, memoryMhz: 0 }];
  const onTargets = Array.from({ length: 5 }, (_, mode) => [true, false].map(onAc => ({ mode, onAc, unavailableReason: null, target: mode === 4
    ? { lockMhz: null, offsets: null } : mode === 2 && !onAc ? { lockMhz: null, offsets: [{ coreMhz: 0, memoryMhz: 0 }] }
      : { lockMhz: [700, 900, 1700, 1400][mode], offsets: [saved[0]] } }))).flat();
  return { identity: "isolated-gpu-turbo", current: { source: { mode: 2, onAc: true, supplyError: null,
    capabilities: { limitClock: true, allModeOc: true }, limits: { entertainmentMhz: 1400, performanceMhz: 1700, savingMhz: 900, quietMhz: 700, thermalMhz: 650 }, saved },
    devices: [{ id, offsets, coreEditable: true, memoryEditable: true, coreRangeKhz: [-300000, 300000], memoryRangeKhz: [-1000000, 1000000] }],
    otherDevices: [], locks, baseMhz: null, off: { lockMhz: 650, offsets: null }, offError: null,
    currentOn: onTargets[4].target, onTargets }, error: null, initial: null, restoreCurrent: null, restoreRevision: null, restoreError: null };
}
export function batteryThresholdFixture() {
  return { identity: "isolated-battery-thresholds", current: {
    active: "381b4222f69441f09685ff5bb260df2e",
    plans: [{ id: "381b4222f69441f09685ff5bb260df2e", values: [[10, 11], [5, 7], [4, 6]] },
      { id: "a184130835414fabbc81f71556f20b4a", values: [[9, 0], [2, 3], [1, 4]] }], cache: [0, 0, 0],
  }, error: null, recovery: null, restoreRevision: null, restoreError: null };
}
export function lowBatteryBrightnessFixture() {
  return { identity: "isolated-batteryLowBrightness", current: {
    rule: { version: [1, 0], customer: 0, feature: true }, panel: "isolated-panel",
    plan: "381b4222f69441f09685ff5bb260df2e", levels: [80, 60], cache: [0, 0],
  }, settingTarget: [0, 0], endingTarget: [80, 60], onAc: false, percent: 20, error: null, recovery: null, restoreRevision: null, restoreError: null };
}
export function restoreLowBatteryBrightnessFixture(data, args, emit) {
  const profile = data.hotkeys.facts.batteryLowBrightness, recovery = profile.recovery;
  if (!recovery || args.expectedRevision !== profile.restoreRevision) throw new Error("低电量亮度恢复预览已变化");
  if (data.lowBatteryRestoreFault) { const error = data.lowBatteryRestoreFault; data.lowBatteryRestoreFault = null; throw new Error(error); }
  recovery.record.levelsOwned.forEach((owned, index) => { if (owned) profile.current.levels[index] = recovery.record.baseline.levels[index]; });
  recovery.record.cacheOwned.forEach((owned, index) => { if (owned) profile.current.cache[index] = recovery.record.baseline.cache[index]; });
  profile.recovery = null; profile.restoreRevision = null; data.hotkeys.status.running = false;
  if (data.hotkeys.facts.batteryAlert) data.hotkeys.facts.batteryAlert.brightness = structuredClone(profile);
  data.hotkeys.revision += "-low-brightness-restored"; emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;
}
export function restoreBatteryThresholdFixture(data, args, emit) {
  const battery = data.hotkeys.facts.batteryThresholds, recovery = battery.recovery;
  if (!recovery || args.expectedRevision !== battery.restoreRevision) throw new Error("电池阈值初始恢复预览已变化");
  if (data.batteryThresholdRestoreFault) { const error = data.batteryThresholdRestoreFault; data.batteryThresholdRestoreFault = null; throw new Error(error); }
  recovery.baseline.plans.forEach((plan, index) => plan.values.forEach((pair, level) => pair.forEach((value, supply) => {
    if (recovery.owned[index][level][supply]) battery.current.plans[index].values[level][supply] = value;
  })));
  recovery.cacheOwned.forEach((owned, index) => { if (owned) battery.current.cache[index] = recovery.baseline.cache[index]; });
  battery.recovery = null; battery.restoreRevision = null; data.hotkeys.status.running = false;
  data.hotkeys.revision += "-battery-restored"; emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;
}
export async function restorePdEventFixture(data, args, emit) {
  const pd = data.hotkeys.facts.pdEvent?.current;
  if (!pd?.initial || pd.revision !== args.expectedRevision || pd.blockedReason || pd.restoreBlockedReason) throw new Error("PD初始恢复预览已变化或不可执行");
  if (data.pdEventRestoreFault) { const error = data.pdEventRestoreFault; data.pdEventRestoreFault = null; throw new Error(error); }
  await invokePlatform(data, "restore_pd_level", args);
  data.hotkeys.status.running = false; data.hotkeys.revision += "-pd-restored";
  emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;
}
export function dismissColdWarningFixture(data, args, emit) {
  if (data.hotkeys.status.coldWarning?.id !== args.expectedId) throw new Error("低温提醒已变化，请重新查看");
  if (data.coldWarningDismissFault) { const error = data.coldWarningDismissFault; data.coldWarningDismissFault = null; throw new Error(error); }
  data.hotkeys.status.coldWarning = null; emit("hotkeys-state", data.hotkeys.status); return null;
}
export async function acknowledgeNavigationFixture(data, args, emit) {
  const request = data.hotkeys.status.navigationRequest;
  if (!request?.ready || request.id !== args.expectedId) throw new Error("热键导航请求已变化");
  if (data.navigationAckDelay) await new Promise(resolve => setTimeout(resolve, data.navigationAckDelay));
  if (data.hotkeys.status.navigationRequest?.id !== args.expectedId) throw new Error("热键导航请求已变化");
  if (data.navigationAckFault) { const error = data.navigationAckFault; data.navigationAckFault = null; throw new Error(error); }
  data.hotkeys.status.navigationRequest = null; emit("hotkeys-state", data.hotkeys.status); return null;
}
export function initializeHotkeys(data) {
  data.hotkeys ??= { revision: "hotkeys-0", recordError: null, resetRevision: null, current: { enabled: false, actions: [], osd: true }, initial: null,
    facts: { capabilities: ["powerMode", "fan", "keyboardBrightness", "keyboardSwitch", "keyboardZones", "keyboardColor", "microphoneMute", "touchpad", "fnLock", "mappings", "openConsole", "stateNotifications", "keyboardBrightnessEvent", "cameraReports", "screenBrightness", "panelHotkey", "gpuTurbo", "batteryThresholds", "powerDeliveryEvent", "batteryColdWarning", "batteryLowBrightness", "batteryLowAction", "openKeyboard", "displayRotation", "batteryAlert", "systemRecovery", "systemShutdown", "lidAction", "airplaneMode", "panelEvent"].map(scope => scope === "powerDeliveryEvent" && !data.platform.powerDelivery ? { scope, identity: null, reason: "固件未声明PD档位控制" } : scope === "openKeyboard" && ![1, 2, 3, 6, 22, 23, 38].includes(data.keyboard.detectedType) ? { scope, identity: null, reason: "当前键盘没有原厂键盘灯快捷入口" } : scope === "keyboardBrightnessEvent" && ![1, 22].includes(data.keyboard.detectedType) ? { scope, identity: null, reason: "此键盘未提供原厂亮度循环或反馈" } : scope === "keyboardColor" && ![6, 22, 38].includes(data.keyboard.detectedType) ? { scope, identity: null, reason: "此键盘未提供原厂颜色循环" } : scope === "keyboardZones" && data.keyboard.detectedType !== 2 ? { scope, identity: null, reason: "此键盘未提供原厂分区开关" } : { scope, identity: `isolated-${scope}`, reason: null }), blockedReason: null },
    authorized: false, status: { running: false, error: null, lastResult: null, coldWarning: null, navigationRequest: null }, osdError: null };
  data.hotkeys.facts.lidAction ??= { identity: "isolated-lid-action", current: { plan: "381b4222f69441f09685ff5bb260df2e", actions: ["screenOff", "hibernate"], modernStandby: true }, onAc: true, error: null, powerError: null };
  data.hotkeys.facts.modeCycle ??= data.modeCycleFixture ?? null;
  data.hotkeys.facts.screenBrightness ??= { identity: "isolated-screen-policy", current: {
    reading: { panel: "isolated-panel", current: 70, onAc: false }, plan: "381b4222f69441f09685ff5bb260df2e",
    levels: [80, 60], targets: [40, 35, 90, 80, 70, 60, 50, 40, 30, 20, 0],
  }, error: null, initial: [], restoreRevision: null, restoreError: null };
  data.hotkeys.facts.panelHotkey ??= { identity: "isolated-panelHotkey", current: {
    kind: "pointerWake", inventory: { monitors: [{ instance: "DISPLAY\\fixture", name: "隔离面板", hardwareIds: ["fixture"],
      signature: "01020304", fingerprint: "a".repeat(64), oled: false, status: 0, problem: 0 }], unresolved: [] }, hookFlags: [1, 0, 0], saver: null,
  }, error: null, session: null, restoreRevision: null };
  data.hotkeys.facts.gpuTurbo ??= gpuTurboFixture();
  data.hotkeys.facts.batteryThresholds ??= batteryThresholdFixture();
  data.hotkeys.facts.batteryLowBrightness ??= lowBatteryBrightnessFixture();
  data.hotkeys.facts.batteryLowAction ??= { identity: "isolated-batteryLowAction", current: { plan: "381b4222f69441f09685ff5bb260df2e", low: 1, critical: 2 }, onAc: false, percent: 10, error: null, powerError: null };
  data.hotkeys.facts.batteryAlert ??= { identity: "isolated-batteryAlert", binding: {
    brightness: "isolated-batteryLowBrightness", customer: 0, lowControl: true, pcBeep: true, codec: "isolated-codec" },
    brightness: structuredClone(data.hotkeys.facts.batteryLowBrightness), playing: false, error: null,
    codec: { current: { endpoint: "isolated-render", volumePart: "pc-beep-volume", mutePart: "pc-beep-mute", muted: true },
      initial: null, error: null, restoreRevision: null, restoreError: null } };
  data.hotkeys.facts.pdEvent ??= data.platform.powerDelivery ? { identity: "isolated-powerDeliveryEvent", current: data.platform.powerDelivery, error: null } : null;
}
export function restoreBatteryAlertFixture(data, args, emit) {
    const alert = data.hotkeys.facts.batteryAlert;
    if (!alert.codec.initial || args.expectedRevision !== alert.codec.restoreRevision) throw new Error("电池音频恢复预览已变化");
    data.hotkeys.status.running = false; alert.playing = false;
    if (data.batteryAlertRestoreFault) { const error = data.batteryAlertRestoreFault; data.batteryAlertRestoreFault = null; throw new Error(error); }
    alert.codec.current = structuredClone(alert.codec.initial); alert.codec.initial = null; alert.codec.restoreRevision = null;
    alert.error = null; data.hotkeys.revision += "-audio-restored";
    emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;

}
export function stopHotkeysFixture(data, emit) {
  data.hotkeys.status.running = false; data.hotkeys.status.coldWarning = null;
  data.hotkeys.facts.panelHotkey.session = null; data.hotkeys.facts.panelHotkey.restoreRevision = null;
  const alert = data.hotkeys.facts.batteryAlert;
  if (alert && data.batteryAlertStopFault) {
    const error = data.batteryAlertStopFault; data.batteryAlertStopFault = null; alert.error = error;
    emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); throw new Error(error);
  }
  if (alert) {
    alert.playing = false; alert.error = null;
    if (alert.codec.initial) {
      alert.codec.current = structuredClone(alert.codec.initial); alert.codec.initial = null; alert.codec.restoreRevision = null;
    }
  }
  emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;
}
export async function invokeHotkeys(data, command, args, emit) {
  initializeHotkeys(data);
  if (command === "reset_invalid_hotkeys") {
    if (!data.hotkeys.recordError || !args.expectedRevision || args.expectedRevision !== data.hotkeys.resetRevision) throw new Error("不可用热键策略已变化");
    if (data.hotkeysResetFault) throw new Error(data.hotkeysResetFault);
    data.hotkeys.current = { enabled: false, actions: [], osd: true };
    data.hotkeys.recordError = null; data.hotkeys.resetRevision = null; data.hotkeys.initial = null;
    data.hotkeys.authorized = false; data.hotkeys.status.running = false;
    data.hotkeys.revision = "hotkeys-reset";
    emit("hotkeys-state", data.hotkeys.status); return null;
  }
  if (command === "read_hotkey_status") {
    const snapshot = structuredClone(data.hotkeys.status);
    if (data.coldWarningStatusDelay) await new Promise(resolve => setTimeout(resolve, data.coldWarningStatusDelay));
    return snapshot;
  }
  if (command === "dismiss_battery_cold_warning") return dismissColdWarningFixture(data, args, emit);
  if (command === "acknowledge_hotkey_navigation") return acknowledgeNavigationFixture(data, args, emit);
  if (command === "restore_pd_event") return restorePdEventFixture(data, args, emit);
  if (command === "restore_battery_thresholds") return restoreBatteryThresholdFixture(data, args, emit);
  if (command === "restore_battery_alert_audio") return restoreBatteryAlertFixture(data, args, emit);
  if (command === "restore_low_battery_brightness") return restoreLowBatteryBrightnessFixture(data, args, emit);
  if (command === "restore_gpu_turbo") {
    const gpu = data.hotkeys.facts.gpuTurbo;
    if (!gpu.initial || args.expectedRevision !== gpu.restoreRevision) throw new Error("GPU Turbo 初始恢复预览已变化");
    if (data.gpuTurboRestoreFault) { const error = data.gpuTurboRestoreFault; data.gpuTurboRestoreFault = null; throw new Error(error); }
    if (gpu.current && gpu.initial.locks) gpu.current.locks = structuredClone(gpu.initial.locks);
    gpu.initial.offsets.forEach((value, index) => {
      const current = gpu.current?.devices[index]?.offsets;
      if (current) { if (value.coreKhz !== null) current.coreKhz = value.coreKhz; if (value.memKhz !== null) current.memKhz = value.memKhz; }
    });
    gpu.initial = null; gpu.restoreCurrent = null; gpu.restoreRevision = null;
    data.hotkeys.status.running = false; data.hotkeys.revision += "-gpu-restored";
    emit("hotkeys-state", data.hotkeys.status); emit("hardware-controls-changed", null); return null;
  }
  if (command === "end_panel_hotkey") {
    const panel = data.hotkeys.facts.panelHotkey;
    if (!panel.session || args.expectedRevision !== panel.restoreRevision) throw new Error("屏幕按键会话预览已变化");
    if (data.panelHotkeyEndFault) { const error = data.panelHotkeyEndFault; data.panelHotkeyEndFault = null; throw new Error(error); }
    panel.session = null; panel.restoreRevision = null; data.hotkeys.status.running = false;
    data.hotkeys.revision += "-panel-ended"; emit("hotkeys-state", data.hotkeys.status); return null;
  }
  if (command === "restore_screen_brightness") {
    const screen = data.hotkeys.facts.screenBrightness;
    if (args.expectedRevision !== screen.restoreRevision) throw new Error("初始屏幕计划亮度预览已变化");
    if (data.screenBrightnessRestoreFault) { const error = data.screenBrightnessRestoreFault; data.screenBrightnessRestoreFault = null; throw new Error(error); }
    for (const initial of screen.initial) {
      if (initial.current) initial.current = initial.current.map((value, i) => initial.owned[i] ? initial.original[i] : value);
      if (screen.current?.plan === initial.plan) screen.current.levels = [...initial.current];
    }
    screen.restoreRevision += "-restored";
    data.hotkeys.status.running = false; data.hotkeys.revision += "-brightness-restored";
    emit("hotkeys-state", data.hotkeys.status);
    return null;
  }
  if (command === "read_osd") return structuredClone(data.osd ?? { sequence: 0, revision: 0, message: null, visible: false,
    error: null, appearance: data.preferences, appearanceError: null });
  if (command === "preview_osd") { data.osdPreviews = (data.osdPreviews ?? 0) + 1; return null; }
  if (command === "stop_hotkeys") return stopHotkeysFixture(data, emit);
  if (command === "read_hotkeys") {
    if (data.hotkeysReadFault) throw new Error(data.hotkeysReadFault);
    return structuredClone(data.hotkeys);
  }
  const hotkeys = data.hotkeys, expected = command === "apply_hotkeys" ? args.request.expectedRevision : args.expectedRevision;
  if (expected !== hotkeys.revision) throw new Error("热键策略或设备预览已变化");
  if (data.hotkeysFault) { const error = data.hotkeysFault; data.hotkeysFault = null; throw new Error(error); }
  hotkeys.initial ??= structuredClone(hotkeys.current);
  hotkeys.facts.panelHotkey.session = null; hotkeys.facts.panelHotkey.restoreRevision = null;
  hotkeys.current = structuredClone(command === "apply_hotkeys" ? args.request.settings : hotkeys.initial);
  hotkeys.authorized = hotkeys.current.enabled;
  hotkeys.status = { running: hotkeys.current.enabled, error: null, lastResult: null, coldWarning: null, navigationRequest: null };
  data.hotkeyRevision = (data.hotkeyRevision ?? 0) + 1;
  hotkeys.revision = `hotkeys-${data.hotkeyRevision}`;
  return null;
}

