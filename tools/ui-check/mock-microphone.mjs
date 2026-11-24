export function invokeMicrophones(data, command, args) {
  data.microphones ??= { revision: "microphone-0", current: { endpoints: [
    { id: "mic-A", name: "内置麦克风", muted: false }, { id: "mic-B", name: "USB麦克风", muted: true },
  ], saved: 0 }, blockedReason: null };
  const profile = data.microphones;
  profile.blockedReason = !profile.current.endpoints.length ? "没有活动麦克风" : data.microphoneBlockedReason || null;
  if (command === "read_microphones") {
    if (data.microphoneReadFault) throw new Error(data.microphoneReadFault);
    return structuredClone(profile);
  }
  if (command !== "apply_microphones") throw new Error("未知麦克风命令：" + command);
  if (profile.revision !== args.request.expectedRevision) throw new Error("麦克风预览已变化");
  if (profile.blockedReason) throw new Error(profile.blockedReason);
  applyMicrophoneTarget(data, { endpoints: profile.current.endpoints.map(endpoint => ({ ...endpoint, muted: args.request.muted })), savedMuted: args.request.muted });
  return null;
}
export function applyMicrophoneTarget(data, target) {
  const profile = data.microphones;
  if (profile.blockedReason || !profile.current.endpoints.length) throw new Error(profile.blockedReason || "没有活动麦克风");
  if (JSON.stringify(target.endpoints.map(e => e.id)) !== JSON.stringify(profile.current.endpoints.map(e => e.id))) throw new Error("档案麦克风集合已变化");
  if (target.endpoints.every((endpoint, index) => endpoint.muted === profile.current.endpoints[index].muted) && Number(target.savedMuted) === profile.current.saved) return false;
  const before = structuredClone(profile.current);
  profile.current.endpoints = profile.current.endpoints.map((endpoint, index) => ({ ...endpoint, muted: target.endpoints[index].muted }));
  profile.current.saved = Number(target.savedMuted);
  profile.revision = "microphone-" + (data.microphoneRevision = (data.microphoneRevision ?? 0) + 1);
  if (data.microphoneWriteFault) {
    if (!data.microphoneRecoveryFault) profile.current = before;
    throw new Error(data.microphoneWriteFault + (data.microphoneRecoveryFault ? "；恢复失败" : "；本次前态已恢复"));
  }
  return true;
}
export function applyProfileMicrophones(data, settings) {
  const target = settings.microphones;
  if (!target) return false;
  invokeMicrophones(data, "read_microphones", {});
  return applyMicrophoneTarget(data, target);
}
