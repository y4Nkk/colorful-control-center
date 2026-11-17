export function captureProfileDevices(data) {
  if (data.display.pending) throw new Error("先结束当前显示试用");
  if (data.dust && !data.dust.current) throw new Error("原厂未报告除尘计划，无法捕获；请先明确保存计划");
  const settings = { ...structuredClone(data.profile.settings), graphicsMode: captureProfileGraphicsMode(data), oemPowerMode: data.performanceOptions.currentMode,
    lightBar: data.lightBar?.current ?? null, dustSchedule: data.dust?.current ?? null,
    keyboard: Object.values(data.keyboardSettings.current).some(value => value !== null) ? data.keyboardSettings.current : null,
    multiZoneLighting: captureMultiZone(data), keyboardLighting: null, lightingOptions: null, perkeyLighting: null, keymap: null, powerDelivery: null, usbPolicy: null, batteryCharge: null, batteryEnergy: null,
    display: data.display.displays.length ? { displays: data.display.displays.map(display => structuredClone(display.current)) } : null,
    performanceOptions: Object.values(data.performanceOptions.current).some(value => value !== null) ? data.performanceOptions.current : null,
    lanPolicy: captureProfileLan(data), gpuPower: captureProfileGpuPower(data) };
  if (settings.gpuPower && !settings.gpuPower.values.enabled) settings.gpu = null;
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
  const microphones = invokeMicrophones(data, "read_microphones", {}).current;
  if (microphones.endpoints.length && microphones.saved > 1) throw new Error("原厂静音偏好未知，无法捕获");
  settings.microphones = microphones.endpoints.length ? { endpoints: microphones.endpoints, savedMuted: microphones.saved !== 0 } : null;
  const touchpad = invokeTouchpad(data, "read_touchpad", {}).current;
  if (touchpad.devices.length && (touchpad.saved > 1 || touchpad.enabled === null)) throw new Error("触摸板状态或偏好未知，无法捕获");
  settings.touchpad = touchpad.devices.length ? { devices: touchpad.devices, enabled: touchpad.enabled, savedEnabled: touchpad.saved !== 0 } : null;
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
  if (data.keymap) {
    if (!data.keymap.configuration || data.keymap.recoveryRequired) throw new Error("映射来源不可读或上次操作待恢复，无法捕获");
    const configuration = structuredClone(data.keymap.configuration);
    configuration.devices = configuration.devices.filter(device => !data.keymapOwned || data.keymapOwned.includes(device.device));
    for (const device of configuration.devices) device.enabled = data.keymap.enabledFlags[device.device === "keyboard" ? 0 : 1] === 1;
    settings.keymap = { configuration, programsEnabled: data.keymap.programs.running };
  }
  return structuredClone(settings);
}
