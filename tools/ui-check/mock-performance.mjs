import { modePreviewTarget, modeRestoreTarget, modeRestoreIssue, rememberMode, applyModeGpu } from "./mock-mode-state.mjs";

export async function invokePerformance(data, command, args) {
  const p = data.performanceOptions;
  const energy = () => {
    if (data.modeEnergy.hardwarePState !== (data.modeEnergy.energyStar === null) || !data.modeEnergy.hardwarePState && ![0, 1, 2].includes(data.modeEnergy.energyStar)) throw new Error("固件节能状态不可恢复");
    return structuredClone(data.modeEnergy);
  };
  const restoreBlockedReason = () => data.modeInitial?.energy && data.modeInitial.energy.hardwarePState !== energy().hardwarePState ? "固件节能管理能力已变化，不能恢复此初始模式" : modeRestoreIssue(data);
  const restore = () => modeRestoreTarget(data);
  const unsupported = [20720, 20724].includes(data.unsupportedModeProduct);
  const blockedReason = unsupported ? `尚未支持${data.unsupportedModeProduct}机型的完整电源模式控制` : data.calibration.workerRunning || data.calibration.recoveryRequired
    ? "电池校准正在接管或尚需恢复，暂不能切换电源模式及其关联策略"
    : data.dust?.running ? "固件正在除尘，请等待结束" : null;
  switch (command) {
    case "read_oem_power_mode": return { currentMode: p.currentMode, acMode: data.acMode, energy: unsupported ? null : energy(), restore: restore(), availableModes: unsupported ? [] : [0, 1, 2, 3], restoreBlockedReason: restoreBlockedReason(),
      canRestore: Boolean(data.modeInitial) && !restoreBlockedReason(), softwareActive: false, blockedReason,
      targets: unsupported ? [] : [0, 1, 2, 3].map(mode => modePreviewTarget(data, mode)) };
    case "read_oem_performance_options":
      if (data.performanceReadFault) throw new Error(data.performanceReadFault);
      return structuredClone({ ...p, acMode: data.modeInitial?.acMode != null ? data.acMode : null,
        energy: data.modeInitial?.energy != null ? energy() : null,
        restore: restore(), restoreBlockedReason: restoreBlockedReason(), canRestore: Boolean(data.modeInitial) && !restoreBlockedReason(), blockedReason });
    case "apply_oem_performance_options":
      if (blockedReason) return { ok: false, message: blockedReason };
      if (args.request.expectedRevision !== p.revision) return { ok: false, message: "性能选项预览已变化" };
      if (data.performanceFault) return { ok: false, message: data.performanceFault };
      rememberMode(data, false); p.current = structuredClone(args.request.values); p.canRestore = true;
      p.revision = `options-${++data.performanceRevision}`; return { ok: true, message: "已接受" };
    case "apply_oem_power_mode":
      if (blockedReason) return { ok: false, message: blockedReason };
      if (args.expected.acMode !== args.expected.mode) return { ok: false, message: "插电偏好目标未预览" };
      if (JSON.stringify(args.expected.energy) !== JSON.stringify((await invokePerformance(data, "read_oem_power_mode", {})).targets.find(target => target.mode === args.expected.mode)?.energy)) return { ok: false, message: "固件节能目标已变化" };
      if (JSON.stringify(args.expected) !== JSON.stringify(modePreviewTarget(data, args.expected.mode))) return { ok: false, message: "完整模式预览已变化，请重新读取" };
      if (args.expected.unavailableReason) return { ok: false, message: args.expected.unavailableReason };
      if (data.modeApplyFault) return { ok: false, message: data.modeApplyFault };
      if (args.expected.gpu?.status !== "ready") return { ok: false, message: "模式GPU预览不可执行" };
      if (args.expected.gpu.value.clocks.candidate.decision?.target && !args.expected.gpu.value.clocks.candidate.projected) return { ok: false, message: "隔离夹具未注入确认后的GPU读数" };
      rememberMode(data, true, args.expected); applyModeGpu(data, args.expected);
      data.acMode = args.expected.acMode; p.currentMode = args.expected.mode; p.canRestore = true;
      data.modeEnergy = structuredClone(args.expected.energy);
      data.fan.currentMode = args.expected.fanMode; data.fan.offsetPct = args.expected.fanOffsetPct; data.fan.software = null;
      p.revision = `options-${++data.performanceRevision}`; return { ok: true, message: "模式已应用" };
    case "restore_oem_power_mode": {
      const target = restore();
      if (restoreBlockedReason()) return { ok: false, message: restoreBlockedReason() };
      if (!target || args.expectedRevision !== target.revision) return { ok: false, message: "初始模式恢复目标已变化，请重新预览并确认" };
      p.currentMode = target.mode; p.current = structuredClone(data.modeInitial.options); p.canRestore = false;
      if (target.acMode !== null) data.acMode = target.acMode;
      if (target.energy !== null) data.modeEnergy = structuredClone(target.energy);
      data.fan.currentMode = target.fan.mode; data.fan.offsetPct = target.fan.offset; data.fan.software = structuredClone(target.fan.software);
      if (target.gpu?.node) {
        const node = data.gpuPower.devices[0]; node.enabled = target.gpu.node.target.enabled;
        node.automaticPowerOff = target.gpu.node.target.automaticPowerOff; node.problem = node.enabled ? 0 : 22;
      }
      if (target.gpu?.clocks) data.modeGpuCurrent = structuredClone(target.gpu.clocks.state);
      for (const plan of target.systemPlans) data.modeSystemValues[plan.scheme] = structuredClone(plan.initial);
      data.modeInitial = null; p.revision = `options-${++data.performanceRevision}`;
      return { ok: true, message: "初始模式与选项已恢复" };
    }
  }
  throw new Error(`未声明的原厂模式命令：${command}`);
}
