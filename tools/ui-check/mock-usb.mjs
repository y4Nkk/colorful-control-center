export function invokeUsb(data, command, args) {
  if (command === "read_usb_policy") {
    if (data.usbReadFault) throw new Error(data.usbReadFault);
    const value = structuredClone(data.usb);
    const active = value.plans.find(plan => plan.id === value.activePlan);
    if (active) value.activeValues = { ac: active.ac, dc: active.dc };
    if (data.calibration.workerRunning || data.calibration.recoveryRequired) {
      value.blockedReason = value.restoreBlockedReason = "电池校准期间不能修改系统电源计划";
    }
    return value;
  }
  const profile = data.usb;
  const expected = args.request?.expectedRevision ?? args.expectedRevision;
  if (expected !== profile.revision) throw new Error("USB 选择性暂停、系统计划或固件建议已变化，请重新读取后重试");
  const restoring = command === "restore_usb_policy";
  if (restoring ? profile.restoreBlockedReason : profile.blockedReason) throw new Error(restoring ? profile.restoreBlockedReason : profile.blockedReason);
  if (data.calibration.workerRunning || data.calibration.recoveryRequired) throw new Error("电池校准期间不能修改系统电源计划");
  if (restoring) {
    for (const initial of profile.initial) {
      const current = profile.plans.find(plan => plan.id === initial.id);
      if (!current) throw new Error("初始快照对应的计划已删除");
      for (const field of ["ac", "dc"]) if (initial[field] !== null) current[field] = initial[field];
    }
  } else {
    if (!profile.supported) throw new Error("固件未声明 USB 选择性暂停");
    const target = args.request.values;
    for (const plan of profile.plans) {
      for (const field of ["ac", "dc"]) {
        if (target[field] === null || target[field] === plan[field]) continue;
        if (plan[field] === null) throw new Error("USB原始设置未知");
        let initial = profile.initial.find(value => value.id === plan.id);
        if (!initial) { initial = { id: plan.id, ac: null, dc: null }; profile.initial.push(initial); }
        initial[field] ??= plan[field];
        if (!data.usbFault) plan[field] = target[field];
      }
    }
  }
  if (data.usbFault) { const error = data.usbFault; data.usbFault = null; throw new Error(error); }
  profile.revision = `usb-${data.usbRevision = (data.usbRevision ?? 0) + 1}`;
  return null;
}
