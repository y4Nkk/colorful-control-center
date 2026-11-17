export function captureMultiZone(data) {
  if (!data.multiZone) return null;
  if (!data.multiZone.current) throw new Error("多区键盘保存值未知，不能捕获");
  return structuredClone({ capabilities: data.multiZone.capabilities, values: data.multiZone.current });
}
export function invokeMultiZone(data, command, args) {
  const profile = data.multiZone;
  if (command === "read_multi_zone_lighting") {
    if (data.multiZoneReadFault) throw new Error(data.multiZoneReadFault);
    return structuredClone(profile ?? null);
  }
  if (!profile) throw new Error("多区键盘不可用");
  const revision = command === "apply_multi_zone_lighting" ? args.request.expectedRevision : args.expectedRevision;
  if (revision !== profile.revision) throw new Error("多区键盘预览已变化");
  if (profile.blockedReason) throw new Error(profile.blockedReason);
  if (command === "restore_multi_zone_lighting") {
    if (!profile.initial) throw new Error("没有多区初始快照");
    if (JSON.stringify(profile.capabilities) !== JSON.stringify(data.multiZoneInitialCapabilities)) throw new Error("多区键盘能力已变化");
    profile.current = structuredClone(profile.initial);
  } else {
    if (!profile.current) throw new Error("多区保存值未知");
    const target = args.request.target;
    if (JSON.stringify(target.capabilities) !== JSON.stringify(profile.capabilities)) throw new Error("多区键盘目标能力已变化");
    const before = structuredClone(profile.current);
    profile.initial ??= before;
    data.multiZoneInitialCapabilities ??= structuredClone(profile.capabilities);
    profile.current = structuredClone(target.values); profile.canRestore = true;
    if (data.multiZoneWriteFault) {
      if (!data.multiZoneRecoveryFault) profile.current = before;
      profile.revision = `multi-${++data.multiZoneRevision}`;
      throw new Error(data.multiZoneWriteFault + (data.multiZoneRecoveryFault ? "；恢复失败" : "；本次设置已恢复"));
    }
  }
  profile.revision = `multi-${++data.multiZoneRevision}`;
}
export function applyProfileMultiZone(data) {
  const target = data.profile.settings.multiZoneLighting;
  if (!target) return false;
  const current = captureMultiZone(data);
  if (!current || JSON.stringify(current.capabilities) !== JSON.stringify(target.capabilities)) throw new Error("档案多区键盘能力已变化");
  if (JSON.stringify(current.values) === JSON.stringify(target.values)) return false;
  invokeMultiZone(data, "apply_multi_zone_lighting", { request: { expectedRevision: data.multiZone.revision, target } });
  return true;
}
