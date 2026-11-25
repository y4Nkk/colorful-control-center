import { invokePerformance } from "./mock-performance.mjs";
import { optionCommand, rememberMode, applyModeFan, applyModeGpu, modePreviewTarget } from "./mock-mode-state.mjs";

export function profileNodeStages(target, settings) {
  const gpu = target.gpu;
  if (gpu?.status !== "ready" || !gpu.value.node) return null;
  const before = structuredClone(gpu.value.node), explicit = settings.gpuPower?.values;
  const needsGpu = settings.gpu !== null || settings.fanSoftwareCurves !== null;
  if (needsGpu && explicit?.enabled === false) throw new Error("GPU目标需要最终保持独显开启");
  let current = structuredClone(before);
  const step = wanted => {
    if (!wanted || current.enabled === wanted.enabled && current.automaticPowerOff === wanted.automaticPowerOff) return null;
    if (!wanted.enabled && (current.drivesDisplay || !current.canDisable)) throw new Error("独显正在驱动显示输出，不能关闭");
    const request = { expected: structuredClone(current), target: structuredClone(wanted), restore: false };
    Object.assign(current, wanted, { problem: wanted.enabled ? 0 : 22 });
    return request;
  };
  const preparation = step(!before.enabled && (needsGpu || explicit?.enabled) ? { enabled: true, automaticPowerOff: before.automaticPowerOff } : null);
  const beforeMode = step(gpu.value.action?.stage === "beforeMode" ? gpu.value.action.target : null);
  const afterMode = step(gpu.value.action?.stage === "afterMode" ? gpu.value.action.target : null);
  const finish = step(explicit ?? (needsGpu ? { enabled: true, automaticPowerOff: before.automaticPowerOff } : null));
  return { before, mode: target.mode, preparation, beforeMode, afterMode, finish };
}

export function applyProfileMode(data, target) {
  if (!target.gpu || !target.revision || target.fromMode === null) throw new Error("档案需要完整GPU模式确认");
  if (target.gpu.status !== "ready") throw new Error(target.gpu.value);
  rememberMode(data, true, target);
  const graph = target.gpu.value.stages;
  const requests = graph ? [graph.preparation, graph.beforeMode, graph.afterMode, graph.finish].filter(Boolean) : [];
  if (requests.length) {
    if (data.gpuPowerFault) throw new Error(data.gpuPowerFault);
    data.profileGpuAuthorizations = (data.profileGpuAuthorizations ?? 0) + 1;
    data.modeInitial.gpu ??= { node: null, clocks: null, driverIdentity: null };
    data.modeInitial.gpu.node ??= { id: graph.before.id, name: graph.before.name, target: { enabled: graph.before.enabled, automaticPowerOff: graph.before.automaticPowerOff } };
  }
  const actual = structuredClone(target);
  const clocks = actual.gpu.value.clocks;
  if (clocks.candidate.decision?.target && !clocks.driver) {
    if (!data.profileModeAfterEnable) throw new Error("隔离夹具缺少开启后GPU驱动实读");
    Object.assign(clocks, structuredClone(data.profileModeAfterEnable));
    data.modeInitial.gpu ??= { node: null, clocks: null, driverIdentity: null };
    data.modeInitial.gpu.clocks ??= { origin: "afterEnable", state: structuredClone(clocks.driver.current) };
    data.modeInitial.gpu.driverIdentity ??= clocks.driver.identity;
  }
  applyModeGpu(data, actual);
  if (requests.length) {
    const last = requests.at(-1);
    Object.assign(data.gpuPower.devices[0], last.target, { problem: last.target.enabled ? 0 : 22 });
    data.profileNodeSteps = structuredClone(requests);
  }
  data.acMode = target.acMode;
  data.performanceOptions.currentMode = target.mode;
  data.modeOptionsCommand = optionCommand(target.mode, target.turbo, target.dtt);
  data.performanceOptions.canRestore = true;
  data.performanceOptions.revision = `options-${++data.performanceRevision}`;
  data.modeEnergy = structuredClone(target.energy);
  applyModeFan(data, target);
}
export async function previewProfileMode(data, settings) {
  const mode = settings.oemPowerMode;
  if (mode === null) return null;
  if (data.profileModeFault) throw new Error(data.profileModeFault);
  const facts = await invokePerformance(data, "read_oem_power_mode", {});
  let target = facts.targets.find(target => target.mode === mode);
  if (!target) throw new Error("当前设备不能预览此档案的电源模式");
  if (settings.performanceOptions) target = modePreviewTarget(data, mode, settings.performanceOptions);
  if (target.gpu?.status !== "ready") throw new Error(target.gpu?.value ?? "档案模式GPU事实缺失");
  target.gpu.value.stages = profileNodeStages(target, settings);
  if (target.dtt && data.performanceOptions.dttDriverAvailable !== true) throw new Error("未确认Dynamic Tuning驱动，不能激活DTT");
  target.revision = JSON.stringify([target, data.acMode, data.modeEnergy, data.performanceOptions.current]);
  return target;
}
