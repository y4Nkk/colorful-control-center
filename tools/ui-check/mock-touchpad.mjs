export function invokeTouchpad(data, command, args) {
  data.touchpad ??= { revision: "touchpad-0", current: { devices: ["ACPI\\I2C0001\\0"], enabled: true, saved: 0 },
    initial: null, canRestore: false, blockedReason: null, restoreBlockedReason: null };
  const p = data.touchpad;
  const changed = p.initial && JSON.stringify(p.current.devices) !== JSON.stringify(p.initial.devices);
  p.restoreBlockedReason = changed || p.current.enabled === null ? "初始触摸板无法恢复，记录保留" : null;
  p.blockedReason = !p.current.devices.length || p.current.enabled === null ? "Windows触摸板状态未知" : data.touchpadBlockedReason || (changed ? "触摸板设备集合已变化" : null);
  p.canRestore = !!p.initial && !p.restoreBlockedReason && !data.touchpadBlockedReason;
  if (command === "read_touchpad") { if (data.touchpadReadFault) throw new Error(data.touchpadReadFault); return structuredClone(p); }
  const revision = command === "apply_touchpad" ? args.request.expectedRevision : args.expectedRevision;
  if (revision !== p.revision) throw new Error("触摸板预览已变化");
  if (command === "apply_touchpad") {
    applyTouchpadTarget(data, { devices: p.current.devices, enabled: args.request.enabled, savedEnabled: args.request.enabled });
    return null;
  }
  if (command === "restore_touchpad") {
    if (!p.canRestore) throw new Error(p.restoreBlockedReason || "没有可恢复的触摸板状态");
    if (data.touchpadRestoreFault) throw new Error(data.touchpadRestoreFault);
    p.current = structuredClone(p.initial); p.initial = null; p.canRestore = false;
    p.revision = `touchpad-${data.touchpadRevision = (data.touchpadRevision ?? 0) + 1}`;
    return null;
  }
  throw new Error("未知触摸板命令：" + command);
}
export function applyTouchpadTarget(data, target) {
  const p = data.touchpad;
  if (p.blockedReason || !p.current.devices.length || p.current.enabled === null) throw new Error(p.blockedReason || "触摸板状态未知");
  if (JSON.stringify(target.devices) !== JSON.stringify(p.current.devices)) throw new Error("档案触摸板设备已变化");
  if (target.enabled === p.current.enabled && Number(target.savedEnabled) === p.current.saved) return false;
  const before = structuredClone(p.current);
  p.initial ??= before; p.canRestore = true;
  p.current.enabled = target.enabled; p.current.saved = Number(target.savedEnabled);
  p.revision = `touchpad-${data.touchpadRevision = (data.touchpadRevision ?? 0) + 1}`;
  if (data.touchpadWriteFault) {
    if (!data.touchpadRecoveryFault) p.current = before;
    throw new Error(data.touchpadWriteFault + (data.touchpadRecoveryFault ? "；恢复失败，记录保留" : "；本次前态已恢复"));
  }
  return true;
}
export function applyProfileTouchpad(data, settings) {
  const target = settings.touchpad;
  if (!target) return false;
  invokeTouchpad(data, "read_touchpad", {});
  return applyTouchpadTarget(data, target);
}
