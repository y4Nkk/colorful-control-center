export async function invokeLan(data, command, args) {
  const clone = value => structuredClone(value), profile = data.lan;
  if (command === "read_lan_policy") {
    if (data.lanReadFault) throw new Error(data.lanReadFault);
    return clone(profile ?? null);
  }
  const request = args.request ?? { expectedRevision: args.expectedRevision, deviceId: args.deviceId };
  if (!profile || request.expectedRevision !== profile.revision) throw new Error("LAN预览、驱动或恢复记录已变化");
  const device = profile.frame.devices.find(value => value.identity.instance === request.deviceId);
  if (!device) throw new Error("原LAN设备已移除");
  const string = value => value?.kind === 1 ? String.fromCharCode(...value.bytes.filter((_, index) => index % 2 === 0)).replace(/\0$/, "") : null;
  const value = text => ({ kind: 1, bytes: [...text, "\0"].flatMap(letter => [letter.charCodeAt(0), 0]) });
  let changes;
  if (command === "restore_lan_policy") {
    const initial = profile.initial.find(item => item.identity.instance === request.deviceId);
    if (!initial || JSON.stringify(initial.identity) !== JSON.stringify(device.identity)) throw new Error("LAN原驱动已变化，保留恢复记录");
    changes = Object.entries(initial.fields).map(([field, target]) => ({ field, before: device.values[field], target }));
  } else {
    const target = {};
    if (request.powerSaving !== null) {
      if (!profile.frame.capabilities.powerSaving || !["0", "1"].includes(string(device.values.powerSavingMode))) throw new Error("LAN节能字段未知或未声明能力");
      target.powerSavingMode = value(request.powerSaving ? "1" : "0");
    }
    if (request.applyWakePatch) {
      if (!profile.frame.capabilities.wakePatch || !["s5WakeOnLan", "shutdownWake", "wakeOnSlot"].some(field => string(device.values[field]) !== null)) throw new Error("不符合唤醒补丁条件");
      target.pnpCapabilities = { kind: 4, bytes: [0, 1, 0, 0] };
      if (string(device.values.wakeOnSlot) !== null) target.wakeOnLink = value("2");
    }
    changes = Object.entries(target).map(([field, target]) => ({ field, before: device.values[field], target }));
  }
  changes = changes.filter(change => JSON.stringify(change.before) !== JSON.stringify(change.target));
  if (command === "preview_lan_policy") {
    if (data.lanPreviewFault) throw new Error(data.lanPreviewFault);
    await Promise.resolve();
    return clone(changes);
  }
  if (profile.blockedReason || data.calibration.workerRunning || data.calibration.recoveryRequired) throw new Error(profile.blockedReason ?? "电池校准正在接管");
  for (const change of changes) {
    if (command === "apply_lan_policy") {
      let initial = profile.initial.find(item => item.identity.instance === request.deviceId);
      if (!initial) { initial = { identity: clone(device.identity), fields: {} }; profile.initial.push(initial); }
      if (!(change.field in initial.fields)) initial.fields[change.field] = clone(change.before);
    }
    if (!data.lanFault) device.values[change.field] = clone(change.target);
    if (data.lanFault) { const error = data.lanFault; data.lanFault = null; profile.lastError = error; throw new Error(error); }
  }
  profile.lastError = null; profile.revision = `lan-${data.lanRevision = (data.lanRevision ?? 0) + 1}`;
  return null;
}
