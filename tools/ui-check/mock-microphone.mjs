export function invokeMicrophones(data, command, args) {
  data.microphones ??= { revision: "microphone-0", current: { endpoints: [
    { id: "mic-A", name: "内置麦克风", muted: false }, { id: "mic-B", name: "USB麦克风", muted: true },
  ], saved: 0 }, initial: null, canRestore: false, blockedReason: null, restoreBlockedReason: null };
  const profile = data.microphones, ids = value => value.endpoints.map(endpoint => endpoint.id).join("\n");
  profile.restoreBlockedReason = profile.initial && profile.initial.endpoints.some(old => !profile.current.endpoints.some(current => current.id === old.id)) ? "初始麦克风已断开，不能恢复到其他设备" : null;
  profile.blockedReason = !profile.current.endpoints.length ? "没有活动麦克风" : data.microphoneBlockedReason || (profile.initial && ids(profile.current) !== ids(profile.initial) ? "活动麦克风集合已变化，不接管新设备" : null);
  profile.canRestore = !!profile.initial && !profile.restoreBlockedReason && !data.microphoneBlockedReason;
  if (command === "read_microphones") {
    if (data.microphoneReadFault) throw new Error(data.microphoneReadFault);
    return structuredClone(profile);
  }
  const expected = command === "apply_microphones" ? args.request.expectedRevision : args.expectedRevision;
  if (profile.revision !== expected) throw new Error("麦克风预览已变化");
  if (command === "apply_microphones" && profile.blockedReason) throw new Error(profile.blockedReason);
  if (command === "apply_microphones") {
    applyMicrophoneTarget(data, { endpoints: profile.current.endpoints.map(endpoint => ({ ...endpoint, muted: args.request.muted })), savedMuted: args.request.muted });
    return null;
  }
  if (command === "restore_microphones") {
    if (profile.restoreBlockedReason || data.microphoneBlockedReason || !profile.initial) throw new Error(profile.restoreBlockedReason || data.microphoneBlockedReason || "没有麦克风初始状态");
    if (data.microphoneRestoreFault) throw new Error(data.microphoneRestoreFault);
    profile.current.endpoints = profile.current.endpoints.map(endpoint => ({ ...endpoint, muted: profile.initial.endpoints.find(old => old.id === endpoint.id)?.muted ?? endpoint.muted }));
    profile.current.saved = profile.initial.saved; profile.initial = null; profile.canRestore = false;
    profile.revision = "microphone-" + (data.microphoneRevision = (data.microphoneRevision ?? 0) + 1);
    return null;
  }
  throw new Error("未知麦克风命令：" + command);
}
export function applyMicrophoneTarget(data, target) {
  const profile = data.microphones;
  if (profile.blockedReason || !profile.current.endpoints.length) throw new Error(profile.blockedReason || "没有活动麦克风");
  if (JSON.stringify(target.endpoints.map(e => e.id)) !== JSON.stringify(profile.current.endpoints.map(e => e.id))) throw new Error("档案麦克风集合已变化");
  if (target.endpoints.every((endpoint, index) => endpoint.muted === profile.current.endpoints[index].muted) && Number(target.savedMuted) === profile.current.saved) return false;
  const before = structuredClone(profile.current);
  profile.initial ??= before; profile.canRestore = true;
  profile.current.endpoints = profile.current.endpoints.map((endpoint, index) => ({ ...endpoint, muted: target.endpoints[index].muted }));
  profile.current.saved = Number(target.savedMuted);
  profile.revision = "microphone-" + (data.microphoneRevision = (data.microphoneRevision ?? 0) + 1);
  if (data.microphoneWriteFault) {
    if (!data.microphoneRecoveryFault) profile.current = before;
    throw new Error(data.microphoneWriteFault + (data.microphoneRecoveryFault ? "；恢复失败，初始状态保留" : "；本次前态已恢复"));
  }
  return true;
}
export function applyProfileMicrophones(data, settings) {
  const target = settings.microphones;
  if (!target) return false;
  invokeMicrophones(data, "read_microphones", {});
  return applyMicrophoneTarget(data, target);
}
