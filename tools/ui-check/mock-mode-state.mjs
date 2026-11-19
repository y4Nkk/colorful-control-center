export function modeGpuReview(data, mode) {
  const node = structuredClone(data.gpuPower.devices[0] ?? null);
  const override = data.modeGpuOverrides?.[mode];
  if (override) {
    const result = structuredClone(override);
    if (result.status === "ready") {
      result.value.node = node;
      if (result.value.clocks.driver && data.modeGpuCurrent) result.value.clocks.driver.current = structuredClone(data.modeGpuCurrent);
    }
    return result;
  }
  const onAc = data.modeSupply === undefined ? data.battery.power.acOnline : data.modeSupply;
  if (node?.automaticPowerOff && [1, 4].includes(mode) && (node.drivesDisplay || !node.canDisable)) return { status: "unavailable", value: "独显正在驱动显示输出，不能关闭" };
  const changesNode = node && ([0, 2, 3].includes(mode) || node.automaticPowerOff && [1, 4].includes(mode));
  return { status: "ready", value: { nodeSupported: node !== null, node, onAc, stages: null,
    action: changesNode ? { stage: [2, 3].includes(mode) ? "beforeMode" : "afterMode", target: { enabled: [0, 2, 3].includes(mode), automaticPowerOff: node.automaticPowerOff } } : null,
    clocks: { policy: { capabilities: { limitClock: false, allModeOc: false }, limits: { entertainmentMhz: 0, performanceMhz: 0, savingMhz: 0, quietMhz: 0, thermalMhz: 0 },
      saved: [{ coreMhz: 0, memoryMhz: 0 }, { coreMhz: 0, memoryMhz: 0 }], gpuOc: false, simpleGpuOc: false, ready: false, gpuCount: 1, cachedDeviceId: 0 },
      candidate: { decision: { target: null, skipped: mode === 4 ? "quietPlus" : "notReady" }, unavailableReason: null, projected: null }, driver: null, driverError: null, deferredReason: null } } };
}
export function appliesPerformanceOptions(data, mode) {
  return mode === 2 || mode === 3 && data.entertainmentOptions === true
    && Object.values(data.performanceOptions.current).some(value => value !== null);
}

export function optionCommand(mode, turbo, dtt) {
  return (mode === 4 ? 0 : mode) | (turbo ? 64 : 0) | (dtt ? 128 : 0);
}

export function entertainmentCommand(data) {
  return (data.modeOptionsCommand & 63) === 3 ? data.modeOptionsCommand : 3;
}

export function modePreviewTarget(data, mode) {
  const p = data.performanceOptions;
  const active = appliesPerformanceOptions(data, mode);
  const turbo = active && p.current.turboFan === true;
  const fanSavedMode = mode === 2 && !turbo ? data.performanceFanMode ?? 0 : 0;
  const fanMode = mode === 3 && !turbo && data.entertainmentQuiet ? 8 : fanSavedMode;
  const target = { mode, fromMode: p.currentMode, acMode: mode, gpu: modeGpuReview(data, mode), unavailableReason: null,
    energy: data.modeEnergy.hardwarePState ? { hardwarePState: true, energyStar: null } : { hardwarePState: false, energyStar: [2, 1, 0, 2, 1][mode] },
    turbo, dtt: active && p.current.dtt === true, fanMode, fanSavedMode, fanOffsetPct: 0, aspm: null, powerPlan: null };
  return { ...target, revision: JSON.stringify([target, data.acMode, data.modeEnergy, p.current, data.fan.currentMode, data.fan.offsetPct, data.fan.software]) };
}
export function modeRestoreTarget(data) {
  const before = data.modeInitial;
  if (!before) return null;
  const device = data.gpuPower.devices[0];
  const entertainmentQuiet = before.fan.fields.find(field => field.offset === 9);
  const target = { mode: before.mode, acMode: before.acMode, turbo: before.options.turboFan === true, dtt: before.options.dtt === true,
    entertainmentQuiet: entertainmentQuiet ? entertainmentQuiet.data[0] === 1 : null, fan: before.fan, system: null, energy: before.energy,
    systemPlans: (before.systemPlans ?? []).map(plan => ({ ...plan, current: data.modeSystemValues?.[plan.scheme] ?? null })),
    gpu: before.gpu ?? null, currentGpuPower: before.gpu && device ? structuredClone(device) : null,
    currentGpuClocks: before.gpu?.clocks ? device?.enabled ? data.modeGpuCurrent ? { status: "ready", value: data.modeGpuCurrent } : { status: "unavailable", value: "隔离夹具缺少恢复频率实读" } : { status: "poweredOff" } : { status: "untracked" },
    currentGpuDriverIdentity: before.gpu?.clocks && device?.enabled ? before.gpu.driverIdentity : null };
  return { ...structuredClone(target), revision: JSON.stringify([target, before.command]) };
}
export function modeRestoreIssue(data) {
  const target = modeRestoreTarget(data);
  if (target?.systemPlans.some(plan => plan.current === null)) return "系统计划恢复前态不可读";
  if (target?.currentGpuClocks.status === "unavailable") return target.currentGpuClocks.value;
  const node = data.gpuPower.devices[0], initial = target?.gpu?.node;
  if (initial && initial.id !== node?.id) return "独显恢复基线属于其他设备";
  if (initial && !initial.target.enabled && (node.drivesDisplay || !node.canDisable)) return "独显正在驱动显示输出，不能恢复关闭状态";
  return data.modeRestoreFault ?? null;
}
export function rememberMode(data, selection, target) {
  const p = data.performanceOptions;
  const fanFields = [{ offset: 5, data: [data.fan.currentMode] }, { offset: 7, data: [data.fan.offsetPct] }];
  if (data.performanceFanMode !== null) fanFields.push({ offset: 70, data: [0, 0, data.performanceFanMode, 0, 0, 0, 0, 0] });
  const command = p.currentMode === 3 ? entertainmentCommand(data)
    : optionCommand(p.currentMode, p.currentMode === 2 && p.current.turboFan, p.currentMode === 2 && p.current.dtt);
  const fanMode = p.currentMode === 3 && data.fan.currentMode === 0 && data.entertainmentQuiet && !(command & 64) ? 8 : data.fan.currentMode;
  data.modeInitial ??= { mode: p.currentMode, command, acMode: null, energy: null, gpu: null, options: structuredClone(p.current),
    fan: { mode: fanMode, offset: data.fan.offsetPct, software: structuredClone(data.fan.software), package: null, rpmPairs: [],
      fields: fanFields } };
  if (!selection) return;
  if (data.modeInitial.acMode === null) data.modeInitial.acMode = data.acMode;
  if (data.modeInitial.energy === null) data.modeInitial.energy = structuredClone(data.modeEnergy);
  if (target?.gpu?.status !== "ready") return;
  const { node, action, clocks } = target.gpu.value;
  if (node && action && node.enabled !== action.target.enabled) {
    data.modeInitial.gpu ??= { node: null, clocks: null, driverIdentity: null };
    data.modeInitial.gpu.node ??= { id: node.id, name: node.name, target: { enabled: node.enabled, automaticPowerOff: node.automaticPowerOff } };
  }
  if (clocks.driver && clocks.candidate.projected && JSON.stringify(clocks.driver.current) !== JSON.stringify(clocks.candidate.projected)) {
    data.modeInitial.gpu ??= { node: null, clocks: null, driverIdentity: null };
    data.modeInitial.gpu.clocks ??= { origin: "beforeMode", state: structuredClone(clocks.driver.current) };
    data.modeInitial.gpu.driverIdentity ??= clocks.driver.identity;
  }
}
export function applyModeGpu(data, target) {
  if (target.gpu.status !== "ready") throw new Error(target.gpu.value);
  const { action, clocks } = target.gpu.value;
  if (clocks.candidate.decision?.target && !clocks.candidate.projected) throw new Error("隔离夹具未注入确认后的GPU读数");
  if (action) {
    const node = data.gpuPower.devices[0];
    node.enabled = action.target.enabled;
    node.automaticPowerOff = action.target.automaticPowerOff;
    node.problem = node.enabled ? 0 : 22;
  }
  if (clocks.candidate.projected) data.modeGpuCurrent = structuredClone(clocks.candidate.projected);
}
