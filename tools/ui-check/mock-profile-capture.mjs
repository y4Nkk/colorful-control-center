export function profileFields() {
  return {
    performance: ["oemPowerMode", "performanceOptions", "fanMode", "fanSoftwareCurves", "fanRpmCurves", "fanCurves",
      "pl1W", "pl2W", "coreOffsetMv", "gtOffsetMv", "runtimeCpu", "gpu"],
    display: ["display", "gpuPower", "graphicsMode"],
    lighting: ["lightBar", "multiZoneLighting", "keyboardLighting", "lightingOptions", "perkeyLighting"],
    power: ["powerDelivery", "usbPolicy", "batteryCharge", "batteryEnergy", "lanPolicy"],
    devices: ["dustSchedule", "keyboard", "microphones", "touchpad", "keymap"],
  };
}

export function emptyProfileSettings() {
  return Object.fromEntries(Object.values(profileFields()).flat().map(key => [key, null]));
}

export function profileGroups(settings) {
  return Object.entries(profileFields()).filter(([, keys]) => keys.some(key => settings[key] != null)).map(([group]) => group);
}

export function captureProfileDevices(data, groups) {
  const settings = emptyProfileSettings();
  if (groups.includes("performance")) {
    const { pl1W, pl2W, coreOffsetMv, gtOffsetMv, runtime, msrRuntimeAvailable } = data.power;
    const { current, currentMode } = data.performanceOptions;
    const saved = { pl1W, pl2W, coreOffsetMv, gtOffsetMv, oemPowerMode: currentMode };
    Object.assign(settings, saved, { fanMode: data.fan.software ? null : data.fan.currentMode, fanSoftwareCurves: data.fan.software,
      performanceOptions: Object.values(current).some(value => value !== null) ? current : null,
      gpu: data.gpu.available ? { coreOffsetMhz: data.gpu.coreOffsetMhz, memOffsetMhz: data.gpu.memOffsetMhz } : null });
    if (msrRuntimeAvailable) settings.runtimeCpu = { pl1W: runtime.pl1W, pl2W: runtime.pl2W,
      coreOffsetUnits: runtime.coreOffsetMv == null ? null : Math.round(runtime.coreOffsetMv * 1.024),
      gtOffsetUnits: runtime.gtOffsetMv == null ? null : Math.round(runtime.gtOffsetMv * 1.024) };
    if (data.fan.currentMode === 6 && data.fan.curveKind === "twoPoint") settings.fanCurves = Object.fromEntries(
      data.fan.fans.filter(fan => fan.present).map(({ id, t2, d2Pct, t3, d3Pct }) => [id, { t2, d2Pct, t3, d3Pct }]));
    if (data.fan.currentMode === 6 && data.fan.curveKind === "rpmTable") settings.fanRpmCurves = Object.fromEntries(
      data.fan.rpmFans.map(fan => [fan.id, fan.points]));
  }
  if (groups.includes("display")) {
    if (data.display.pending) throw new Error("先结束当前显示试用");
    settings.display = data.display.displays.length ? { displays: data.display.displays.map(display => display.current) } : null;
    settings.gpuPower = captureProfileGpuPower(data);
    settings.graphicsMode = captureProfileGraphicsMode(data);
  }
  if (groups.includes("power")) captureProfilePower(data, settings);
  if (groups.includes("lighting")) captureProfileLighting(data, settings);
  if (groups.includes("devices")) captureProfileInputs(data, settings);
  const available = profileGroups(settings);
  if (!available.length) throw new Error("档案没有设置");
  return { settings: structuredClone(settings), groups: available };
}

export function captureProfilePower(data, settings) {
  if (data.calibration.workerRunning || data.calibration.recoveryRequired) throw new Error("电池校准正在接管或尚需恢复");
  if (data.energySupported && data.energy.recoveryRequired) throw new Error("节能操作尚需恢复，不能捕获电池档案");
  if ((data.battery.charge || data.energySupported) && data.battery.power?.present !== true) throw new Error("未确认电池存在，不能捕获电池档案");
  if (data.energySupported && data.energy.saved) settings.batteryEnergy = { policy: data.energy.saved, capability: data.energy.capability };
  if (data.battery.charge && !settings.batteryEnergy?.policy.enabled) {
    const { current, startValues, stopValues } = data.battery.charge;
    settings.batteryCharge = { values: current, startValues, stopValues };
  }
  const pd = data.platform.powerDelivery;
  if (pd) {
    if (!pd.currentValid || !pd.encoding || !pd.availableLevels.includes(pd.currentLevel)) throw new Error("当前PD档位无法捕获");
    settings.powerDelivery = { level: pd.currentLevel, encoding: pd.encoding, adapterWatts: pd.adapterWatts, availableLevels: pd.availableLevels };
  }
  if (data.usb?.supported) {
    if (!data.usb.plans.some(plan => plan.ac !== null || plan.dc !== null)) throw new Error("USB档案没有已知目标字段");
    settings.usbPolicy = { plans: data.usb.plans };
  }
  settings.lanPolicy = captureProfileLan(data);
}

export function captureProfileLighting(data, settings) {
  settings.lightBar = data.lightBar?.current ?? null;
  settings.multiZoneLighting = captureMultiZone(data);
  const family = data.family, current = data.keyboard.current;
  if (family === "unknown" || !data.keyboard.available) throw new Error("键盘灯状态未知，无法捕获");
  if (family === "zoneRgb" || family === "white") {
    if (!current || family === "zoneRgb" && !current.effect) throw new Error("保存灯效未识别，无法保证恢复");
    settings.keyboardLighting = { keyboardType: data.keyboard.detectedType, enabled: current.enabled,
      settings: family === "white" ? { family, brightness: current.brightnessLevel } : {
        family, brightness: current.brightnessLevel, effect: current.effect, allColor: current.allColor, zones: current.zones } };
  }
  if (family === "perkey") {
    if (!data.perkey?.current) throw new Error("逐键状态未知，无法捕获");
    settings.perkeyLighting = { keyboardType: data.keyboard.detectedType, device: data.perkey.device, values: data.perkey.current };
  }
  if (family !== "none") settings.lightingOptions = {
    keyboardType: data.lightingOptions.capabilities.keyboardType, values: data.lightingOptions.current };
}

export function captureProfileInputs(data, settings) {
  if (data.dust && !data.dust.current) throw new Error("原厂未报告除尘计划，无法捕获；请先明确保存计划");
  settings.dustSchedule = data.dust?.current ?? null;
  settings.keyboard = Object.values(data.keyboardSettings.current).some(value => value !== null) ? data.keyboardSettings.current : null;
  const microphones = invokeMicrophones(data, "read_microphones", {}).current;
  if (microphones.endpoints.length && microphones.saved > 1) throw new Error("原厂静音偏好未知，无法捕获");
  settings.microphones = microphones.endpoints.length ? { endpoints: microphones.endpoints, savedMuted: microphones.saved !== 0 } : null;
  const touchpad = invokeTouchpad(data, "read_touchpad", {}).current;
  if (touchpad.devices.length && (touchpad.saved > 1 || touchpad.enabled === null)) throw new Error("触摸板状态或偏好未知，无法捕获");
  settings.touchpad = touchpad.devices.length ? { devices: touchpad.devices, enabled: touchpad.enabled, savedEnabled: touchpad.saved !== 0 } : null;
  if (data.keymap) {
    if (!data.keymap.configuration || data.keymap.recoveryRequired) throw new Error("映射来源不可读或上次操作待恢复，无法捕获");
    const configuration = structuredClone(data.keymap.configuration);
    configuration.devices = configuration.devices.filter(device => !data.keymapOwned || data.keymapOwned.includes(device.device));
    for (const device of configuration.devices) device.enabled = data.keymap.enabledFlags[device.device === "keyboard" ? 0 : 1] === 1;
    settings.keymap = { configuration, programsEnabled: data.keymap.programs.running };
  }
}
