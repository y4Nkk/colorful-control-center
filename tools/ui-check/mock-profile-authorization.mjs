import { previewProfileMode } from "./mock-profile-mode.mjs";

export async function profileDigest(value) {
  const hash = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(value)));
  return Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, "0")).join("");
}
export async function backgroundProfileBinding(file, approval) {
  if ((file.settings.oemPowerMode !== null) !== (approval !== null)) throw new Error("后台档案缺少完整模式授权");
  return { profile: file.name, digest: await profileDigest(file), mode: approval ? {
    identity: approval.identity, automaticPowerOff: structuredClone(approval.allowedAutomaticPowerOff),
    driver: approval.clocks.driver?.identity ?? null, driverDeferred: !!approval.clocks.deferredReason,
  } : null };
}
export async function previewProfileAuthorization(data, settings) {
  const target = await previewProfileMode(data, settings);
  if (!target) return null;
  if (data.profileAuthorizationFault) throw new Error(data.profileAuthorizationFault);
  const gpu = target.gpu.value, node = gpu.node;
  const allowedAutomaticPowerOff = node ? [...new Set([node.automaticPowerOff, ...(settings.gpuPower ? [settings.gpuPower.values.automaticPowerOff] : [])])].sort() : null;
  const nodeRules = node ? [false, true].map(automaticPowerOff => {
    const enabled = [0, 2, 3].includes(target.mode) ? true : automaticPowerOff ? false : null;
    return { automaticPowerOff, action: enabled === null ? null : { stage: [2, 3].includes(target.mode) ? "beforeMode" : "afterMode", target: { enabled, automaticPowerOff } } };
  }) : [];
  if (node && nodeRules.some(rule => allowedAutomaticPowerOff.includes(rule.automaticPowerOff) && rule.action?.target.enabled === false) && (node.drivesDisplay || !node.canDisable)) throw new Error("档案未来独显联动不满足当前设备资格");
  let choices = data.profileModeChoices?.filter(choice => choice.mode === target.mode);
  if (!choices) {
    if (gpu.clocks.policy.ready) throw new Error("隔离夹具缺少未来供电目标");
    choices = [true, false].map(onAc => ({ mode: target.mode, onAc, candidate: structuredClone(gpu.clocks.candidate) }));
  }
  if (choices.length !== 2 || choices.some(choice => choice.candidate.unavailableReason)) throw new Error("档案未来供电GPU目标不可执行");
  const clocks = { policy: structuredClone(gpu.clocks.policy), choices: structuredClone(choices), driver: structuredClone(gpu.clocks.driver), driverError: null, deferredReason: gpu.clocks.deferredReason };
  const core = Object.fromEntries(Object.entries(target).filter(([key]) => !["fromMode", "gpu", "revision", "unavailableReason"].includes(key)));
  const identity = await profileDigest([core, gpu.nodeSupported, node ? [node.id, node.name] : null, nodeRules,
    settings.gpuPower, settings.gpu !== null || settings.fanSoftwareCurves !== null, clocks.policy, choices.map(choice => [choice.mode, choice.onAc, choice.candidate.decision])]);
  return { identity, target, clocks, nodeRules, allowedAutomaticPowerOff };
}

export async function invokeStartup(data, command, args) {
  if (command === "get_startup_profile") {
    if (data.startupReadFault) throw new Error(data.startupReadFault);
    const selection = structuredClone(data.startupSelection ?? null);
    const bindingIssue = selection && selection.digest !== await profileDigest(data.profile) ? "档案内容已变化，请重新预览并确认后台规则" : null;
    return { selection, bindingIssue, lastRun: data.startupLastRun ?? null };
  }
  if (command === "set_startup_profile") {
    if (args.expected && JSON.stringify(args.expected) !== JSON.stringify(data.profile)) throw new Error("启动档案已变化，请重新预览后确认");
    const actual = args.expected ? await previewProfileAuthorization(data, args.expected.settings) : null;
    if (JSON.stringify(actual) !== JSON.stringify(args.expectedMode)) throw new Error("启动档案完整模式授权已变化，请重新预览");
    data.startupSelection = args.expected ? await backgroundProfileBinding(args.expected, actual) : null;
    data.startupReadFault = null;
    return structuredClone(data.startupSelection);
  }
  throw new Error(`未声明的启动档案命令：${command}`);
}
