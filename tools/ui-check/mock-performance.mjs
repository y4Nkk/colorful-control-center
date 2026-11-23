import { appliesPerformanceOptions, optionCommand, entertainmentCommand, modePreviewTarget, rememberMode, applyModeGpu } from "./mock-mode-state.mjs";

export function fanProfile(data) {
  const powerMode = data.performanceOptions.currentMode;
  const fan = data.fan;
  const quiet = powerMode === 3 && fan.currentMode === 0 && fan.availableModes.includes(8)
    && data.entertainmentQuiet && !(entertainmentCommand(data) & 64);
  const currentMode = quiet ? 8 : fan.currentMode;
  const offsetSlot = data.fanRemembersOffset ? (powerMode === 4 ? 0 : powerMode) : null;
  const running = data.dust?.running ? "固件正在除尘，请等待结束后再调整风扇或电源模式" : null;
  const offsetBlockedReason = fan.offsetBlockedReason ?? running ?? (fan.software ? "软件接管期间无法调节加速偏移。"
    : [0, 8].includes(currentMode) && fan.availableModes.includes(currentMode) ? null : "仅自动或寂静模式可调节加速偏移。");
  const profile = { ...fan, currentMode, offsetSlot, blockedReason: running, offsetBlockedReason };
  if (data.fanKind !== "rpmTable") return profile;
  return { ...profile, curveKind: "rpmTable", offsetBlockedReason: "此机型不支持风扇加速偏移。", fans: [],
    rpmFans: fan.rpmFans.length ? fan.rpmFans : [{ id: "cpu", maximumRpm: 5000,
      points: Array.from({ length: 10 }, (_, i) => ({ tempC: 40 + i * 5, rpmPct: i === 9 ? 100 : 20 + i * 8 })) }] };
}

export function applyPerformanceOptions(data, { values, quietFan, replaceCustomFan }, quiet) {
  const p = data.performanceOptions;
  const performance = p.currentMode === 2;
  const active = appliesPerformanceOptions(data, p.currentMode);
  const command = optionCommand(p.currentMode, active && values.turboFan, active && values.dtt);
  if (!performance && p.currentMode !== 3) return { ok: false, message: "请在性能模式或娱乐模式配置性能选项" };
  if (quietFan !== null && !quiet) return { ok: false, message: "当前模式或固件未开放寂静模式" };
  if (active && quietFan && values.turboFan) return { ok: false, message: "Turbo 风扇与寂静模式不能同时开启" };
  const beforeCommand = performance ? optionCommand(2, p.current.turboFan, p.current.dtt) : entertainmentCommand(data);
  const turbo = !!(beforeCommand & 64);
  const fanChanged = quietFan !== null && (quiet.enabled !== quietFan || quietFan && !quiet.active) || active && !!values.turboFan !== turbo;
  if (replaceCustomFan) {
    const turboHandoff = !performance && active && !turbo && values.turboFan === true && quietFan !== true;
    const expectedTurbo = turboHandoff ? true : active && quietFan && p.current.turboFan !== null ? false : p.current.turboFan;
    if (!p.customFan || !fanChanged || !turboHandoff && quietFan === null || values.turboFan !== expectedTurbo || values.dtt !== p.current.dtt)
      return { ok: false, message: "自定义风扇切换目标已变化，请重新确认" };
  }
  if ((active || fanChanged) && p.softwareActive || fanChanged && p.customFan && !replaceCustomFan) return { ok: false, message: "请先确认切换风扇曲线" };
  const flagsChanged = values.turboFan !== p.current.turboFan || values.dtt !== p.current.dtt;
  const sendsCommand = active && (flagsChanged || command !== beforeCommand);
  if (sendsCommand && values.dtt && p.dttDriverAvailable !== true) return { ok: false, message: "未确认 Dynamic Tuning 驱动" };
  rememberMode(data, false);
  p.current = structuredClone(values);
  if (sendsCommand) data.modeOptionsCommand = command;
  if (fanChanged) p.customFan = false;
  if (fanChanged && performance) {
    data.fan.currentMode = quietFan ? 8 : 0;
    if (data.performanceFanMode !== null) data.performanceFanMode = data.fan.currentMode;
  }
  if (fanChanged && !performance) {
    const fields = data.modeInitial.fan.fields;
    if (quiet) {
      if (!fields.some(field => field.offset === 9)) fields.push({ offset: 9, data: [Number(data.entertainmentQuiet)] });
      data.entertainmentQuiet = quietFan === true;
    }
    data.fan.currentMode = 0;
  }
  p.revision = `options-${++data.performanceRevision}`;
  return { ok: true, message: "已接受" };
}

export async function invokePerformance(data, command, args) {
  const p = data.performanceOptions;
  const performance = p.currentMode === 2;
  const entertainment = p.currentMode === 3;
  const appliesInCurrentMode = appliesPerformanceOptions(data, p.currentMode);
  const quiet = (performance || entertainment) && data.fan.availableModes.includes(8) ? {
    enabled: entertainment ? data.entertainmentQuiet : (data.performanceFanMode ?? data.fan.currentMode) === 8,
    active: data.fan.currentMode === 8 || entertainment && data.fan.currentMode === 0 && data.entertainmentQuiet && !(entertainmentCommand(data) & 64),
    remembered: entertainment || data.performanceFanMode !== null,
  } : null;
  const energy = () => {
    if (data.modeEnergy.hardwarePState !== (data.modeEnergy.energyStar === null) || !data.modeEnergy.hardwarePState && ![0, 1, 2].includes(data.modeEnergy.energyStar)) throw new Error("固件节能状态不可恢复");
    return structuredClone(data.modeEnergy);
  };
  const unsupported = [20720, 20724].includes(data.unsupportedModeProduct);
  const blockedReason = unsupported ? `尚未支持${data.unsupportedModeProduct}机型的完整电源模式控制` : data.calibration.workerRunning || data.calibration.recoveryRequired
    ? "电池校准正在接管或尚需恢复，暂不能切换电源模式及其关联策略"
    : data.dust?.running ? "固件正在除尘，请等待结束" : null;
  switch (command) {
    case "read_oem_power_mode": return { currentMode: p.currentMode, acMode: data.acMode, energy: unsupported ? null : energy(), availableModes: unsupported ? [] : [0, 1, 2, 3], softwareActive: false, blockedReason,
      targets: unsupported ? [] : [0, 1, 2, 3].map(mode => modePreviewTarget(data, mode)) };
    case "read_oem_performance_options":
      if (data.performanceReadFault) throw new Error(data.performanceReadFault);
      return structuredClone({ ...p, appliesInCurrentMode,
        entertainmentOptionsPending: entertainment && appliesInCurrentMode && entertainmentCommand(data) !== optionCommand(3, p.current.turboFan, p.current.dtt),
        quietFan: quiet, blockedReason });
    case "apply_oem_performance_options":
      if (blockedReason) return { ok: false, message: blockedReason };
      if (args.request.expectedRevision !== p.revision) return { ok: false, message: "性能选项预览已变化" };
      if (data.performanceFault) return { ok: false, message: data.performanceFault };
      return applyPerformanceOptions(data, args.request, quiet);
    case "apply_oem_power_mode":
      if (blockedReason) return { ok: false, message: blockedReason };
      if (args.expected.acMode !== args.expected.mode) return { ok: false, message: "插电偏好目标未预览" };
      if (JSON.stringify(args.expected.energy) !== JSON.stringify((await invokePerformance(data, "read_oem_power_mode", {})).targets.find(target => target.mode === args.expected.mode)?.energy)) return { ok: false, message: "固件节能目标已变化" };
      if (JSON.stringify(args.expected) !== JSON.stringify(modePreviewTarget(data, args.expected.mode))) return { ok: false, message: "模式联动条件已变化，请重新读取后再切换" };
      if (args.expected.unavailableReason) return { ok: false, message: args.expected.unavailableReason };
      if (args.confirmedFnKey && args.confirmedFnKey.modeRevision !== args.expected.revision)
        return { ok: false, message: "确认的模式目标已变化，请重新选择" };
      if (data.fnKey) {
        const confirmation = { identity: data.fnKey, modeRevision: args.expected.revision };
        if (!args.confirmedFnKey) return { confirmFnKey: structuredClone(confirmation) };
        if (JSON.stringify(args.confirmedFnKey.identity) !== JSON.stringify(data.fnKey))
          return { ok: false, message: "原厂 FnKey 已重新启动，请重新选择模式并确认" };
        if (data.fnKeyExitFault) return { ok: false, message: data.fnKeyExitFault };
        data.fnKey = null;
      }
      if (data.modeApplyFault) return { ok: false, message: data.modeApplyFault };
      if (args.expected.gpu?.status !== "ready") return { ok: false, message: "模式GPU预览不可执行" };
      if (args.expected.gpu.value.clocks.candidate.decision?.target && !args.expected.gpu.value.clocks.candidate.projected) return { ok: false, message: "隔离夹具未注入确认后的GPU读数" };
      rememberMode(data, true, args.expected); applyModeGpu(data, args.expected);
      data.acMode = args.expected.acMode; p.currentMode = args.expected.mode;
      data.modeOptionsCommand = optionCommand(p.currentMode, args.expected.turbo, args.expected.dtt);
      data.modeEnergy = structuredClone(args.expected.energy);
      data.fan.currentMode = args.expected.fanSavedMode; data.fan.offsetPct = args.expected.fanOffsetPct; data.fan.software = null;
      p.customFan = data.fan.currentMode === 6;
      if (args.expected.mode === 2 && data.performanceFanMode !== null) data.performanceFanMode = args.expected.fanSavedMode;
      p.revision = `options-${++data.performanceRevision}`; return { ok: true, message: "模式已应用" };
  }
  throw new Error(`未声明的原厂模式命令：${command}`);
}
