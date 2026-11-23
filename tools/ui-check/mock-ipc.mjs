import { invokeDisplay } from "./mock-display.mjs";
import { invokeAirplane } from "./mock-airplane.mjs";
import { invokeAutoRefresh } from "./mock-auto-refresh.mjs";
import { invokeSupplyMode } from "./mock-supply-mode.mjs";
import { applyPerformanceOptions, fanProfile, invokePerformance } from "./mock-performance.mjs";
import { modeGpuReview, appliesPerformanceOptions, optionCommand, entertainmentCommand, modePreviewTarget, rememberMode, applyModeGpu } from "./mock-mode-state.mjs";
import { initPowerPolicy, savePowerPolicy, invokePowerPolicy } from "./mock-profile-policy.mjs";
import { invokeLightingOptions, applyProfileLightingOptions } from "./mock-lighting-options.mjs";
import { invokeTouchpad, applyTouchpadTarget, applyProfileTouchpad } from "./mock-touchpad.mjs";
import { invokeMicrophones, applyMicrophoneTarget, applyProfileMicrophones } from "./mock-microphone.mjs";
import { invokeMultiZone, captureMultiZone, applyProfileMultiZone } from "./mock-multizone.mjs";
import { invokeKeymap } from "./mock-keymap.mjs";
import { stopHotkeysFixture, restoreBatteryAlertFixture, batteryThresholdFixture, dismissColdWarningFixture, gpuTurboFixture, initializeHotkeys, invokeHotkeys, restoreBatteryThresholdFixture, restorePdEventFixture, lowBatteryBrightnessFixture, restoreLowBatteryBrightnessFixture, acknowledgeNavigationFixture } from "./mock-hotkeys.mjs";
import { invokeLightBar } from "./mock-lightbar.mjs";
import { invokeUsb } from "./mock-usb.mjs";
import { invokeLan } from "./mock-lan.mjs";
import { captureProfileDevices, captureProfilePower, captureProfileLighting, captureProfileInputs, profileFields, profileGroups, emptyProfileSettings } from "./mock-profile-capture.mjs";
import { invokeProfileStorage, profileFiles } from "./mock-profile-storage.mjs";
import { previewProfileMode, applyProfileMode, profileNodeStages } from "./mock-profile-mode.mjs";
import { profileDigest, backgroundProfileBinding, previewProfileAuthorization, invokeStartup } from "./mock-profile-authorization.mjs";
import { captureProfileLan, reviewProfileLan } from "./mock-profile-lan.mjs";
import { captureProfileGraphicsMode, previewProfileGraphicsMode, applyProfileGraphicsMode, restoreProfileGraphicsMode } from "./mock-profile-mux.mjs";
import { captureProfileGpuPower, previewProfileGpuPower, applyProfileGpuPower, commitProfileTargets } from "./mock-profile-gpu.mjs";
import { previewProfileDisplay, previewManualProfile, profileApplicationSteps, recoverProfileDisplay, syncProfileApplication, beginProfileApplication, invokeProfileApplication } from "./mock-profile-application.mjs";

function fixtureData() {
  const settings = { ...emptyProfileSettings(), fanMode: 0 };
  const color = { r: 0, g: 100, b: 255 };
  window.__uiFixture = { calls: [], faults: [], fanKind: "twoPoint", family: "zoneRgb", delay: 0, acMode: 2, modeInitial: null, modeEnergy: { hardwarePState: false, energyStar: 2 },
    preferences: { theme: "light", interfaceFont: "system", interfaceScale: 100, motion: "reduce", closeBehavior: "hideToTray",
      alwaysOnTop: false, startMinimized: false, closeOemOnStartup: true, startupPage: "dashboard", globalShortcut: "", runtimeLogLevel: "info", hardwareAcceleration: true },
    profile: { version: 1, name: "隔离档案", savedAtMs: 1, settings }, otherProfiles: [],
    fan: { software: null, curveKind: "twoPoint", rpmFans: [], availableModes: [0, 1, 6], fanCount: 1,
      customSupported: true, currentMode: 0, offsetPct: 0, offsetBlockedReason: null, canRestore: true, warnings: ["隔离风扇诊断"], safetyStatus: "已读取", blockedReason: null,
      fans: [{ id: "cpu", present: true, t1: 30, d1Pct: 20, t2: 60, d2Pct: 50, t3: 90, d3Pct: 99 }] },
    power: { pl1W: 65, pl2W: 100, plTimeS: 28, coreOffsetMv: 0, gtOffsetMv: 0, defaultPl1W: 65, defaultPl2W: 100,
      defaultCoreOffsetMv: 0, msrRuntimeAvailable: true, msrRuntimeError: null, canRestore: true, runtimeWarnings: [], warnings: ["隔离功耗诊断"],
      runtime: { pl1W: 65, pl2W: 100, coreOffsetMv: 0, gtOffsetMv: 0 } },
    gpu: { available: true, unavailableReason: null, gpuName: "Isolated GPU", coreOffsetMhz: 0, memOffsetMhz: 0, coreEditable: true,
      memEditable: true, coreMinMhz: -100, coreMaxMhz: 200, memMinMhz: -200, memMaxMhz: 1000, baseline: null, warnings: [] },
    keyboard: { available: true, unavailableReason: null, detectedType: 6, detectedFamily: "zoneRgb", detectionSource: "隔离夹具", baseline: null, warnings: [],
      current: { brightnessLevel: 3, enabled: true, rawMode: 0, effect: "zones", allColor: color, zones: [0, 1, 2].map(() => ({ enabled: true, color })) } },
    display: { displays: [{ current: { device: "DISPLAY1", identity: "isolated-panel", name: "隔离屏幕", width: 1920, height: 1080,
      bits: 32, orientation: 0, x: 0, y: 0, flags: 0, frequency: 60 }, frequencies: [60, 144], primary: true }], pending: null, canRestore: false, error: null },
    autoRefreshRevision: 0, autoRefreshOwned: null,
    autoRefresh: { revision: "automatic-0", current: { identity: "isolated", display: null, candidates: [60, 144], low: 60, savedOrHighest: 144,
      fields: [0, 0, 144], mode: 1, modes: [0, 1, 2, 3], customer: 1, product: 20600, supplyModeSwitch: false, onAc: true, defaultEnabled: true },
      initial: null, initialDisplay: null, preview: null, active: false, canStart: true, canRestore: false,
      error: null, listenerOnline: true, sleeping: false, issue: null },
    keyboardSettings: { revision: "keyboard-0", current: { fnLocked: false, winLocked: false, fnWinSwap: null }, canRestore: false },
    keyboardInitial: { fnLocked: false, winLocked: false, fnWinSwap: null }, keyboardRevision: 0, keyboardFault: null,
    lightingOptions: { revision: "lighting-options-0", current: { idleTimer: { enabled: true, seconds: 30 }, bootOverride: false, logo: null, lightBar: null , topCover: null},
      capabilities: { keyboardType: 6, peripheralSupportKnown: true, bootOverride: true, logo: false, lightBar: false , topCover: false}, canRestore: false, initial: null },
    multiZone: null, multiZoneRevision: 0, multiZoneReadFault: null, multiZoneWriteFault: null,
    lightingOptionsRevision: 0, lightingOptionsFault: null, lightingOptionsReadFault: null,
    dust: null, dustRevision: 0, dustFault: null, dustReadFault: null, usb: null, lan: null,
    keymap: null, keymapRevision: 0, keymapReadFault: null, keymapFault: null,
    platform: { powerDelivery: null, deepS5: null, warnings: [] }, pdRevision: 0, pdFault: null, platformReadFault: null,
    gpuPower: { devices: [{ id: "ACPI\\NVDA0820\\ISOLATED", name: "隔离原厂独显控制器", enabled: true,
      automaticPowerOff: false, problem: 0, drivesDisplay: true, canDisable: false }], canRestore: false }, gpuPowerFault: null,
    graphicsMode: { current: { mode: 3, availableMask: 15 }, revision: "graphics-0", lastRequest: null, canRestore: false },
    graphicsRevision: 0, graphicsReboots: 0, graphicsModeFault: null, graphicsRebootFault: null,
    battery: { charge: { current: { enabled: false, start: 95, stop: 100 }, startValues: [40, 50, 60, 70, 80, 95], stopValues: [60, 70, 80, 90, 100] },
      revision: "battery-0", chargeLockReason: null, power: { present: true, acOnline: true, charging: false, percent: 62, remainingSeconds: null, windowsBatterySaver: false },
      diagnostics: { version: 1, manufacturedOn: "2024-02-29", cycleCount: 123, fullCapacityRaw: 4800, designCapacityRaw: 5000,
        capacityPct: 96, batteryStatus: 0, permanentFailureStatus: 0, operationStatus: 0, stopChargingThreshold: 80 }, canRestore: false, warnings: [] },
    batteryInitial: { enabled: false, start: 95, stop: 100 }, batteryRevision: 0, batteryFault: null,
    calibrationSupported: false, calibrationFault: null, calibrationRevision: 0,
    calibration: { revision: "calibration-0", task: null, workerRunning: false, controlAcquired: false, canStart: true, canStop: false,
      recoveryRequired: false, stopThreshold: 80, restoreCharge: { enabled: false, start: 95, stop: 100 },
      restoreEnergyEnabled: false, knownEnergyPlan: true, hasEnergyCapability: true, unavailableReason: null, error: null },
    energy: { saved: { enabled: false, everyDay: true, weekdays: 0, slots: [{ start: 8, end: 18 }, null], automatic: true, limits: { floor: 95, ceiling: 100 } },
      capability: { factory: { floor: 95, ceiling: 100 }, factoryDefaulted: false }, revision: "energy-0", source: "oemSaved", canRestore: false,
      recoveryRequired: false, chargeOnStop: { current: { enabled: false, start: 95, stop: 100 }, startValues: [40, 50, 60, 70, 80, 95], stopValues: [60, 70, 80, 90, 100] },
      chargeReported: { enabled: false, start: 95, stop: 100 }, hasInitialPlan: true, initialCharge: { enabled: false, start: 95, stop: 100 }, error: null, canWrite: true }, energySupported: true, energyRevision: 0, energyFault: null, energyRecoveryFault: false,
    performanceOptions: { current: { turboFan: false, dtt: false }, currentMode: 2, customFan: false, softwareActive: false, blockedReason: null,
      dttDriverAvailable: true, revision: "options-0", warnings: [] }, performanceRevision: 0, performanceFault: null, performanceFanMode: null, entertainmentQuiet: false,
    perkey: { revision: "perkey-0", current: { brightness: 3, enabled: true, speed: 1,
      pattern: { kind: "static", colors: Array.from({ length: 120 }, () => ({ r: 20, g: 80, b: 160 })) } },
      device: { newEffects: true, language: 0, narrowBorder: true, threeColumn: false, compact: false, physicalThreeColumn: false },
      presets: [], canRestore: false, reactiveError: null, reactiveRunning: false }, perkeyRevision: 0, perkeyFault: null,
  };
  const data = window.__uiFixture;
  const initialPerkey = data.initialPerkey = structuredClone(data.perkey.current);
  data.initialEnergy = undefined;
  data.initialEnergyCharge = null;
  data.perkey.presets = [{ kind: "static", colors: initialPerkey.pattern.colors }, { kind: "wave", direction: 0, color: null },
    ...["breath", "blink", "random", "fade"].map(kind => ({ kind, color: null })), { kind: "scan", colors: null },
    { kind: "snake", direction: 0, color: null }, { kind: "ripple", reactive: false, color: null }, { kind: "fn", color: { r: 0, g: 100, b: 255 } }];
}

function installIpc() {
  const data = window.__uiFixture, callbacks = new Map(), listeners = new Map();
  const emit = window.__uiEmit = (event, payload) => {
    for (const [id, listener] of listeners) if (listener.event === event) callbacks.get(listener.handler)?.({ event, id, payload: structuredClone(payload) });
  };
  let nextId = 1;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event, id) => listeners.delete(id) };
  const emitSnapshot = () => emit("hardware-snapshot", hardwareSnapshot(data));
  window.__TAURI_INTERNALS__ = {
    transformCallback: callback => { const id = nextId++; callbacks.set(id, callback); return id; },
    unregisterCallback: id => callbacks.delete(id),
    invoke: async (command, args = {}) => {
      data.calls.push({ command, args });
      switch (command) {
        case "read_oem_supply_mode": case "enable_oem_supply_mode": case "resume_oem_supply_mode":
        case "pause_oem_supply_mode": case "restore_oem_supply_mode": return invokeSupplyMode(data, command, args);
        case "read_lan_policy": case "preview_lan_policy": case "apply_lan_policy": case "restore_lan_policy": return invokeLan(data, command, args);
        case "read_usb_policy": case "apply_usb_policy": case "restore_usb_policy": return invokeUsb(data, command, args);
        case "read_light_bar": case "apply_light_bar": case "restore_light_bar": return invokeLightBar(data, command, args);
        case "read_osd": case "preview_osd": case "read_hotkeys": case "read_hotkey_status": case "dismiss_battery_cold_warning": case "acknowledge_hotkey_navigation": case "apply_hotkeys": case "restore_hotkeys": case "reset_invalid_hotkeys": case "stop_hotkeys": case "restore_screen_brightness": case "restore_battery_alert_audio": case "restore_low_battery_brightness": case "end_panel_hotkey": case "restore_gpu_turbo": case "restore_battery_thresholds": case "restore_pd_event": return invokeHotkeys(data, command, args, emit);
        case "plugin:event|listen": { const id = nextId++; listeners.set(id, args); return id; }
        case "plugin:event|unlisten": return null;
        case "read_power_profile_policy": case "save_power_profile_policy": case "set_power_profile_policy_enabled": case "pause_power_profile_policy":
        case "restore_power_profile_policy": case "reload_power_profile_policy": case "reset_power_profile_policy": case "read_automatic_profile_application": return invokePowerPolicy(data, command, args);
        case "read_hardware_snapshot": return hardwareSnapshot(data);
        case "read_system_overview": return { cpu: { vendor: "intel", name: "Isolated CPU" }, memory: { totalBytes: 17179869184, availableBytes: 8589934592 },
          memorySpeedMts: data.memorySpeedMts === undefined ? 5200 : data.memorySpeedMts, volumes: [], warnings: [] };
        case "read_fan_profile": return structuredClone(fanProfile(data));
        case "apply_fan_curve": {
          if (data.fanWriteFault === command || data.dust?.running) return { ok: false, message: "隔离风扇写入失败" };
          for (const fan of data.fan.fans) if (args[fan.id]) Object.assign(fan, args[fan.id]);
          data.fan.currentMode = 6; data.fan.software = null;
          return { ok: true, message: "风扇曲线已写入" };
        }
        case "apply_rpm_fan_curves": {
          if (data.fanWriteFault === command || data.dust?.running) return { ok: false, message: "隔离风扇写入失败" };
          data.fan.rpmFans = fanProfile(data).rpmFans.map(fan => ({ ...fan, points: args.curves[fan.id] ?? fan.points }));
          data.fan.currentMode = 6; data.fan.software = null;
          return { ok: true, message: "转速曲线已写入" };
        }
        case "set_fan_mode": {
          if (data.fanWriteFault === command) return { ok: false, message: "隔离风扇写入失败" };
          if (data.dust?.running || !data.fan.availableModes.includes(args.mode)) return { ok: false, message: "风扇模式不可用" };
          data.fan.currentMode = args.mode;
          data.fan.software = null;
          return { ok: true, message: "风扇模式已写入" };
        }
        case "set_fan_offset": {
          const fan = fanProfile(data);
          if (fan.offsetBlockedReason) return { ok: false, message: fan.offsetBlockedReason };
          if (fan.currentMode !== args.expected.mode) return { ok: false, message: "风扇档位已变化，请重新调整" };
          if (fan.offsetSlot !== args.expected.slot) return { ok: false, message: "风扇偏移记忆槽已变化，请重新调整" };
          if (data.fanWriteFault === command) return { ok: false, message: "隔离风扇写入失败" };
          data.fan.offsetPct = args.pct;
          return { ok: true, message: "风扇偏移已写入" };
        }
        case "read_power_profile": return data.power;
        case "apply_saved_cpu": case "apply_runtime_cpu": {
          const controls = args.controls;
          if (data.cpuWriteFault) { const message = data.cpuWriteFault; data.cpuWriteFault = null; return { ok: false, message }; }
          const values = command === "apply_saved_cpu" ? data.power : data.power.runtime;
          if (controls.pl1W != null) values.pl1W = controls.pl1W;
          if (controls.pl2W != null) values.pl2W = controls.pl2W;
          if (controls.coreOffsetMv != null) values.coreOffsetMv = controls.coreOffsetMv;
          if (controls.gtOffsetMv != null) values.gtOffsetMv = controls.gtOffsetMv;
          data.power.canRestore = true;
          return { ok: true, message: command === "apply_saved_cpu" ? "CPU 设置已保存；即时效果未验证" : "即时 CPU 目标已回读确认" };
        }
        case "restore_power_profile": return { ok: true, message: "已恢复初始设置" };
        case "read_gpu_profile": return data.gpu;
        case "read_touchpad": case "apply_touchpad": case "restore_touchpad": return invokeTouchpad(data, command, args);
        case "read_microphones": case "apply_microphones": case "restore_microphones": return invokeMicrophones(data, command, args);
        case "read_airplane": case "apply_airplane": case "restore_airplane": return invokeAirplane(data, command, args);
        case "restore_fan_default": return { ok: true, message: "隔离恢复完成" };
        case "read_multi_zone_lighting": case "apply_multi_zone_lighting": case "restore_multi_zone_lighting": case "read_lighting_options":
        case "read_keyboard_lighting": case "apply_keyboard_lighting": case "restore_keyboard_lighting":
        case "apply_lighting_options": case "restore_lighting_options":
        case "read_keyboard_settings": case "apply_keyboard_settings": case "restore_keyboard_settings":
        case "read_perkey_lighting": case "apply_perkey_lighting": case "restore_perkey_lighting":
          return invokeLighting(data, command, args);
        case "read_battery_calibration": case "start_battery_calibration":
        case "stop_battery_calibration": case "restore_battery_calibration":
          return invokeCalibration(data, command, args);
        case "read_energy_policy": case "apply_energy_policy": case "restore_energy_policy":
          return invokeEnergy(data, command, args);
        case "read_battery_profile": case "apply_battery_charge": case "restore_battery_charge":
          return invokeBattery(data, command, args);
        case "read_oem_power_mode": case "read_oem_performance_options":
        case "apply_oem_performance_options": case "apply_oem_power_mode":
          return invokePerformance(data, command, args);
        case "read_display_modes": case "apply_display_refresh": case "apply_display_rotation":
        case "keep_display_mode": case "cancel_display_trial": case "restore_display_modes":
          return invokeDisplay(data, command, args);
        case "read_automatic_refresh": case "enable_automatic_refresh": case "restore_automatic_refresh":
          return invokeAutoRefresh(data, command, args, emit);
        case "read_graphics_mode": case "apply_graphics_mode":
        case "restore_graphics_mode": case "reboot_for_graphics_mode":
          return invokeGraphics(data, command, args);
        case "read_gpu_power": case "apply_gpu_power": case "restore_gpu_power":
          return invokeGpuPower(data, command, args, emitSnapshot);
        case "get_startup_profile": case "set_startup_profile": case "get_autostart": case "set_autostart": case "get_oem_startup_notice":
        case "get_backend_info": case "list_profiles": case "load_profile": case "preview_profile_import": case "preview_profile_application": case "preview_profile_authorization":
        case "capture_profile_settings": case "save_profile": case "apply_profile": case "export_profile": case "delete_profile":
        case "preview_profile_display": case "preview_profile_graphics_mode": case "preview_profile_gpu_power": case "read_profile_application": case "keep_profile_display": case "cancel_profile_application":
        case "get_app_preferences": case "set_app_preferences":
        case "list_system_fonts": case "set_shortcut_recording": return invokeSoftware(data, command, args);
        case "read_platform_power": case "apply_pd_level": case "restore_pd_level":
          return invokePlatform(data, command, args);
        case "read_fan_dust": case "start_fan_dust": case "apply_fan_dust_schedule": case "restore_fan_dust_schedule":
          return invokeDust(data, command, args);
        case "read_keymap": case "apply_keymap": case "restore_keymap": case "preview_keymap_import": case "retry_keymap_programs":
        case "start_keymap_recording": case "read_keymap_recording": case "stop_keymap_recording":
          return invokeKeymap(data, command, args);
        default: data.faults.push(command); throw new Error(`未声明的隔离命令：${command}`);
      }
    },
  };
}

async function invokeLighting(data, command, args) {
  switch (command) {
    case "read_multi_zone_lighting": case "apply_multi_zone_lighting": case "restore_multi_zone_lighting": return invokeMultiZone(data, command, args);
    case "read_keyboard_lighting": return structuredClone({ ...data.keyboard, detectedFamily: data.family });
    case "apply_keyboard_lighting": {
      const { request } = args, keyboard = data.keyboard;
      if (!keyboard.current || request.family !== data.family) return { ok: false, message: "键盘灯配置不可用", rollbackError: null };
      keyboard.baseline ??= structuredClone(keyboard.current);
      if (request.brightness > 0) keyboard.current.brightnessLevel = request.brightness;
      keyboard.current.enabled = request.brightness > 0;
      if (request.family === "zoneRgb") {
        keyboard.current.effect = request.effect;
        keyboard.current.allColor = structuredClone(request.allColor);
        keyboard.current.zones = structuredClone(request.zones);
      }
      return { ok: true, message: "键盘灯设置已写入", rollbackError: null };
    }
    case "restore_keyboard_lighting":
      if (!data.keyboard.baseline) return { ok: false, message: "没有初始键盘灯设置", rollbackError: null };
      data.keyboard.current = structuredClone(data.keyboard.baseline);
      data.keyboard.baseline = null;
      return { ok: true, message: "键盘灯设置已恢复", rollbackError: null };
    case "read_lighting_options": case "apply_lighting_options": case "restore_lighting_options": return invokeLightingOptions(data, command, args);
    case "read_keyboard_settings": return structuredClone(data.keyboardSettings);
    case "apply_keyboard_settings":
      if (args.request.expectedRevision !== data.keyboardSettings.revision) throw new Error("键盘预览已变化");
      data.keyboardSettings.canRestore = true;
      if (data.keyboardFault) { const message = data.keyboardFault; data.keyboardFault = null; throw new Error(message); }
      data.keyboardSettings.current = structuredClone(args.request.values);
      data.keyboardSettings.revision = `keyboard-${++data.keyboardRevision}`;
      return null;
    case "restore_keyboard_settings":
      data.keyboardSettings.current = structuredClone(data.keyboardInitial);
      data.keyboardSettings.revision = `keyboard-${++data.keyboardRevision}`;
      return null;
    case "read_perkey_lighting": return structuredClone(data.perkey);
    case "apply_perkey_lighting": {
      const { request } = args;
      if (request.expectedRevision !== data.perkey.revision) throw new Error("逐键预览已变化");
      data.perkey.canRestore = true;
      if (data.perkeyFault) { const error = data.perkeyFault; data.perkeyFault = null; throw new Error(error); }
      data.perkey.current = structuredClone(request.values); data.perkey.revision = `perkey-${++data.perkeyRevision}`;
      data.perkey.reactiveRunning = request.values.enabled && ["ripple", "fn", "fade"].includes(request.values.pattern.kind);
      data.perkey.reactiveError = null; return null;
    }
    case "restore_perkey_lighting":
      data.perkey.current = structuredClone(data.initialPerkey); data.perkey.revision = `perkey-${++data.perkeyRevision}`;
      data.perkey.reactiveRunning = false; data.perkey.reactiveError = null; return null;
  }
}

async function invokeCalibration(data, command, args) {
  switch (command) {
    case "read_battery_calibration": if (data.calibrationReadFault) throw new Error(data.calibrationReadFault); return data.calibrationSupported ? structuredClone(data.calibration) : null;
    case "start_battery_calibration": {
      if (args.expectedRevision !== data.calibration.revision || !data.calibration.canStart) throw new Error("校准预览已变化");
      const cal = data.calibration;
      cal.restoreCharge = structuredClone(data.battery.charge.current);
      cal.restoreEnergyEnabled = data.energy.saved?.enabled ?? null; cal.knownEnergyPlan = data.energy.saved != null;
      cal.canStart = false; cal.workerRunning = true; cal.controlAcquired = true; cal.canStop = true;
      cal.task = { status: "running", phase: "discharge1", elapsedMs: 0, phaseElapsedMs: 0, error: null,
        facts: { present: true, acOnline: false, percent: 62, batteryStatus: 0, permanentFailure: 0, stopThreshold: 80 } };
      if (data.calibrationFault) {
        cal.error = data.calibrationFault; cal.workerRunning = false; cal.canStop = false;
        cal.recoveryRequired = true; cal.task.status = "recoveryRequired";
        const error = data.calibrationFault; data.calibrationFault = null;
        cal.revision = `calibration-${++data.calibrationRevision}`; throw new Error(error);
      }
      data.battery.chargeLockReason = "电池校准正在接管或尚需恢复";
      cal.revision = `calibration-${++data.calibrationRevision}`; return null;
    }
    case "stop_battery_calibration":
    case "restore_battery_calibration": {
      const cal = data.calibration;
      if (args.expectedRevision !== cal.revision) throw new Error("校准预览已变化");
      if (command === "restore_battery_calibration" && !cal.recoveryRequired) throw new Error("无需校准恢复");
      cal.workerRunning = false; cal.canStart = true; cal.canStop = false; cal.recoveryRequired = false; cal.error = null;
      if (cal.task) { cal.task.status = "stopped"; cal.task.error = null; }
      data.battery.charge.current = structuredClone(cal.restoreCharge); data.battery.chargeLockReason = null;
      cal.revision = `calibration-${++data.calibrationRevision}`; return null;
    }
  }
}

async function invokeEnergy(data, command, args) {
  switch (command) {
    case "read_energy_policy": return data.energySupported ? structuredClone({ ...data.energy, chargeReported: data.battery.charge?.current ?? data.energy.chargeReported, canWrite: data.energy.canWrite && data.battery.power.present === true }) : null;
    case "apply_energy_policy": {
      const { request } = args;
      if (request.expectedRevision !== data.energy.revision) throw new Error("节能计划预览已变化");
      if (data.energy.recoveryRequired) throw new Error("上次节能操作尚需恢复");
      data.initialEnergyCharge ??= structuredClone(data.battery.charge.current);
      if (data.initialEnergy === undefined) data.initialEnergy = structuredClone(data.energy.saved);
      data.energy.canRestore = true; data.energy.source = "application";
      if (data.energyFault) { const error = data.energyFault; data.energyFault = null; data.energy.error = error;
        data.energy.recoveryRequired = data.energyRecoveryFault; throw new Error(error); }
      const target = request.policy;
      if (target.enabled && !data.energy.saved?.enabled) data.energy.chargeOnStop.current = structuredClone(data.battery.charge.current);
      data.battery.charge.current = target.enabled ? { enabled: true, start: target.limits.floor, stop: target.limits.ceiling } : structuredClone(data.energy.chargeOnStop.current);
      data.battery.chargeLockReason = target.enabled ? "节能计划管理充电阈值，请先停止节能计划" : null;
      data.battery.revision = `battery-${++data.batteryRevision}`;
      data.energy.saved = structuredClone(target); data.energy.error = null; data.energy.revision = `energy-${++data.energyRevision}`; return null;
    }
    case "restore_energy_policy":
      if (args.expectedRevision !== data.energy.revision) throw new Error("节能恢复预览已变化");
      data.energy.saved = structuredClone(data.initialEnergy); data.energy.chargeOnStop.current = structuredClone(data.initialEnergyCharge);
      data.energy.recoveryRequired = false; data.energy.error = null; data.energy.revision = `energy-${++data.energyRevision}`;
      data.battery.charge.current = structuredClone(data.initialEnergyCharge); data.battery.chargeLockReason = null;
      data.battery.revision = `battery-${++data.batteryRevision}`; return null;
  }
}

async function invokeBattery(data, command, args) {
  const lock = data.battery.chargeLockReason ?? (data.calibration.workerRunning || data.calibration.recoveryRequired ? "电池校准正在接管或尚需恢复"
    : data.energySupported && data.energy.recoveryRequired ? "节能操作尚需恢复" : data.energySupported && data.energy.saved?.enabled ? "节能计划管理充电阈值，请先停止节能计划" : null);
  switch (command) {
    case "read_battery_profile": if (data.batteryReadFault) throw new Error(data.batteryReadFault); return structuredClone({ ...data.battery, chargeLockReason: lock });
    case "apply_battery_charge": {
      if (lock) throw new Error(lock);
      if (args.request.expectedRevision !== data.battery.revision) throw new Error("充电预览已变化");
      if (data.batteryFault) throw new Error(data.batteryFault);
      const values = args.request.values;
      if (data.battery.power.present !== true) throw new Error("未确认电池存在");
      if (values.enabled && (!data.battery.charge.startValues.includes(values.start) || !data.battery.charge.stopValues.includes(values.stop) || values.start >= values.stop)) throw new Error("充电阈值无效");
      data.battery.charge.current = structuredClone(values); data.battery.revision = `battery-${++data.batteryRevision}`;
      data.battery.canRestore = true; return null;
    }
    case "restore_battery_charge":
      if (lock) throw new Error(lock);
      if (args.expectedRevision !== data.battery.revision) throw new Error("恢复预览已变化");
      if (data.batteryFault) throw new Error(data.batteryFault);
      data.battery.charge.current = structuredClone(data.batteryInitial); data.battery.revision = `battery-${++data.batteryRevision}`; return null;
  }
}

async function invokeGraphics(data, command, args) {
  switch (command) {
    case "read_graphics_mode": return structuredClone(data.graphicsMode);
    case "apply_graphics_mode": {
      if (args.request.expectedRevision !== data.graphicsMode.revision) throw new Error("显卡选择预览已变化");
      if (data.graphicsModeFault) { const message = data.graphicsModeFault; data.graphicsModeFault = null; throw new Error(message); }
      const initialMode = data.graphicsMode.lastRequest?.initialMode ?? data.graphicsMode.current.mode;
      data.graphicsMode.lastRequest = { initialMode, target: args.request.target, verification: "pendingReboot" };
      data.graphicsMode.canRestore = args.request.target !== initialMode;
      data.graphicsMode.revision = `graphics-${++data.graphicsRevision}`;
      return null;
    }
    case "restore_graphics_mode":
      if (args.expectedRevision !== data.graphicsMode.revision) throw new Error("显卡选择预览已变化");
      data.graphicsMode.lastRequest.target = data.graphicsMode.lastRequest.initialMode;
      data.graphicsMode.lastRequest.verification = "pendingReboot";
      data.graphicsMode.canRestore = false; data.graphicsMode.revision = `graphics-${++data.graphicsRevision}`;
      return null;
    case "reboot_for_graphics_mode":
      if (args.expectedRevision !== data.graphicsMode.revision) throw new Error("显卡选择预览已变化");
      if (data.graphicsRebootFault) throw new Error(data.graphicsRebootFault);
      ++data.graphicsReboots;
      return null;
  }
}

async function invokeGpuPower(data, command, args, emitSnapshot) {
  switch (command) {
    case "read_gpu_power": return structuredClone(data.gpuPower);
    case "apply_gpu_power": {
      const device = data.gpuPower.devices[0];
      if (JSON.stringify(device) !== JSON.stringify(args.expected)) throw new Error("原厂独显预览已变化");
      data.gpuPowerInitial ??= structuredClone(device);
      data.gpuPower.canRestore = true;
      if (data.gpuPowerFault) { const message = data.gpuPowerFault; data.gpuPowerFault = null; throw new Error(message); }
      device.enabled = args.enabled; device.automaticPowerOff = !args.enabled; device.problem = args.enabled ? 0 : 22;
      emitSnapshot();
      return null;
    }
    case "restore_gpu_power": data.gpuPower.devices[0] = structuredClone(data.gpuPowerInitial); emitSnapshot(); return null;
  }
}

async function invokeSoftware(data, command, args) {
  switch (command) {
    case "get_startup_profile": case "set_startup_profile": return invokeStartup(data, command, args);
    case "get_oem_startup_notice": return data.oemStartupNotice ?? null;
    case "get_autostart": return structuredClone(data.autostart ?? { revision: "login-0", level: "disabled", elevated: false, canRestore: false, legacy: false });
    case "set_autostart": {
      const before = data.autostart ?? { revision: "login-0", level: "disabled", elevated: false, canRestore: false, legacy: false };
      if (args.expectedRevision !== before.revision) throw new Error("登录自启预览已变化，请重新读取");
      if (data.autostartFault) { const error = data.autostartFault; data.autostartFault = null; throw new Error(error); }
      data.autostartInitial ??= structuredClone(before);
      data.autostart = { ...before, revision: `${before.revision}-next`, level: args.restore ? data.autostartInitial.level : args.level,
        legacy: args.restore ? data.autostartInitial.legacy : false, canRestore: !args.restore };
      return structuredClone(data.autostart);
    }
    case "get_backend_info": return { name: "Isolated UI", version: "1.0.1" };
    case "list_profiles": case "load_profile": case "preview_profile_import":
    case "save_profile": case "export_profile": case "delete_profile": return invokeProfileStorage(data, command, args);
    case "preview_profile_application": return previewManualProfile(data, args.settings);
    case "preview_profile_authorization": return previewProfileAuthorization(data, args.settings);
    case "capture_profile_settings": return captureProfileDevices(data, args.groups);
    case "apply_profile": case "preview_profile_display": case "preview_profile_graphics_mode": case "preview_profile_gpu_power": case "read_profile_application":
    case "keep_profile_display": case "cancel_profile_application": return invokeProfileApplication(data, command, args);
    case "get_app_preferences": return { settings: data.preferences, desktopError: null, restartRequired: false };
    case "set_app_preferences":
      await new Promise(resolve => setTimeout(resolve, data.delay)); data.preferences = args.settings;
      return { settings: data.preferences, desktopError: null, restartRequired: false };
    case "list_system_fonts": return ["Arial", "Consolas", "Microsoft YaHei"];
    case "set_shortcut_recording": return null;
  }
}

async function invokePlatform(data, command, args) {
  const pd = data.platform.powerDelivery;
  const blocked = data.calibration.workerRunning || data.calibration.recoveryRequired;
  switch (command) {
    case "read_platform_power":
      if (data.platformReadFault) throw new Error(data.platformReadFault);
      return structuredClone({ ...data.platform, powerDelivery: pd ? { ...pd, blockedReason: blocked ? "电池校准期间不能更改PD档位" : pd.blockedReason } : null });
    case "apply_pd_level": {
      if (!pd || blocked || args.request.expectedRevision !== pd.revision || !pd.availableLevels.includes(args.request.level)) throw new Error("PD预览已变化");
      pd.initial ??= { value: pd.currentLevel, zeroBased: pd.zeroBased };
      pd.currentLevel = args.request.level; pd.currentValid = true; pd.revision = `pd-${++data.pdRevision}`;
      if (data.pdFault) { const error = data.pdFault; data.pdFault = null; throw new Error(error); }
      return null;
    }
    case "restore_pd_level":
      if (!pd?.initial || blocked || pd.restoreBlockedReason || args.expectedRevision !== pd.revision) throw new Error("PD恢复预览已变化");
      pd.currentLevel = pd.initial.value; pd.currentValid = true; pd.revision = `pd-${++data.pdRevision}`; return null;
  }
}

function hardwareSnapshot(data) {
  const sampledAtMs = Date.now();
  return { dchuAvailable: true, oemPowerMode: data.performanceOptions.currentMode,
    cpuTempC: 50, cpuRemoteRaw: 50, sampledAtMs, warnings: ["隔离监控诊断"],
    cpuUtility: data.cpuUtility === undefined ? { valuePct: 42, clockMhz: 2900, sampledAtMs } : structuredClone(data.cpuUtility),
    oemMemory: data.oemMemory === undefined ? { reportedClockMhz: 2400, referenceClockMhz: 3200 } : structuredClone(data.oemMemory),
    fan: { cpuRpm: 1800, gpuRpm: 1500, cpuDutyPct: 30, gpuDutyPct: 25 },
    nvidiaSuspended: !data.gpuPower.devices[0].enabled,
    nvidia: data.gpuPower.devices[0].enabled ? { name: "Isolated GPU", tempC: 45, fanPct: 25, utilizationPct: 30, graphicsClockMhz: 1800, memUsedMb: 1000, memTotalMb: 8000 } : null };
}
function dustProfile(data) {
  const dust = data.dust;
  if (!dust) return null;
  const blocked = data.calibration.workerRunning || data.calibration.recoveryRequired ? "电池校准正在接管或尚需恢复" :
    dust.running ? "固件正在除尘，请等待结束" : dust.scheduleBlockedReason;
  const software = !!data.fan.software;
  return { ...structuredClone(dust), softwareActive: software, scheduleBlockedReason: blocked,
    startBlockedReason: blocked || (software ? "请先退出软件风扇曲线，再开始除尘" : dust.startBlockedReason),
    revision: JSON.stringify([data.dustRevision, dust.revision, blocked, software, dust.running, dust.startBlockedReason]) };
}
async function invokeDust(data, command, args) {
  if (command === "read_fan_dust") {
    if (data.dustReadFault) throw new Error(data.dustReadFault);
    return dustProfile(data);
  }
  const profile = dustProfile(data), dust = data.dust;
  const expected = args.request?.expectedRevision ?? args.expectedRevision;
  if (!profile || expected !== profile.revision) throw new Error("除尘预览已变化");
  if (command === "start_fan_dust") {
    if (profile.startBlockedReason) throw new Error(profile.startBlockedReason);
    dust.lastStart = "sent";
  } else {
    if (profile.scheduleBlockedReason) throw new Error(profile.scheduleBlockedReason);
    if (command === "apply_fan_dust_schedule") {
      const target = args.request.schedule;
      if (profile.softwareActive && (target.onResume || target.weekdays)) throw new Error("软件风扇曲线运行时不能启用除尘计划");
      dust.initial ??= { schedule: structuredClone(dust.current) };
      dust.current = structuredClone(target);
    } else {
      if (!dust.initial) throw new Error("没有初始除尘计划");
      dust.current = structuredClone(dust.initial.schedule);
    }
  }
  ++data.dustRevision;
  if (data.dustFault) { const error = data.dustFault; data.dustFault = null; if (command === "start_fan_dust") dust.lastStart = "uncertain"; throw new Error(error); }
  return null;
}
export const MOCK_IPC = `(() => {${[invokeAirplane, modeGpuReview, appliesPerformanceOptions, optionCommand, entertainmentCommand, modePreviewTarget, rememberMode, applyModeGpu, invokeSupplyMode, profileDigest, backgroundProfileBinding, previewProfileAuthorization, invokeStartup, initPowerPolicy, savePowerPolicy, invokePowerPolicy, fixtureData, installIpc, invokeTouchpad, applyTouchpadTarget, applyProfileTouchpad, invokeMicrophones, applyMicrophoneTarget, applyProfileMicrophones, invokeLightingOptions, applyProfileLightingOptions, invokeLighting, invokeCalibration, invokeEnergy, invokeBattery, applyPerformanceOptions, invokePerformance, invokeDisplay, invokeAutoRefresh, invokeGraphics, invokeGpuPower, invokeSoftware, invokePlatform, hardwareSnapshot, fanProfile, dustProfile, invokeDust, invokeKeymap, batteryThresholdFixture, restoreBatteryThresholdFixture, lowBatteryBrightnessFixture, restoreLowBatteryBrightnessFixture, restorePdEventFixture, dismissColdWarningFixture, acknowledgeNavigationFixture, gpuTurboFixture, initializeHotkeys, invokeHotkeys, invokeLightBar, invokeUsb, invokeLan, invokeMultiZone, captureMultiZone, applyProfileMultiZone, profileFields, profileGroups, emptyProfileSettings, captureProfileDevices, captureProfilePower, captureProfileLighting, captureProfileInputs, profileFiles, invokeProfileStorage, captureProfileGraphicsMode, previewProfileGraphicsMode, applyProfileGraphicsMode, restoreProfileGraphicsMode, captureProfileGpuPower, previewProfileGpuPower, applyProfileGpuPower, commitProfileTargets, profileNodeStages, previewProfileMode, applyProfileMode, captureProfileLan, reviewProfileLan, previewProfileDisplay, previewManualProfile, profileApplicationSteps, recoverProfileDisplay, syncProfileApplication, beginProfileApplication, invokeProfileApplication, restoreBatteryAlertFixture, stopHotkeysFixture].map(fn => fn.toString()).join("\n")}; fixtureData(); installIpc();})();`;

