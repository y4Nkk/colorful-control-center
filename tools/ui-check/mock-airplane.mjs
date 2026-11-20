export function invokeAirplane(data, command, args) {
  data.airplane ??= { revision: "airplane-0", identity: "isolated-airplane", current: { device: "airplane-A", enabled: false }, initial: null, error: null, blockedReason: null, restoreRevision: null, restoreError: null };
  const profile = data.airplane;
  if (command === "read_airplane") return structuredClone(profile);
  if (command === "apply_airplane") {
    if (profile.revision !== args.request.expectedRevision || !profile.current) throw new Error("飞行模式预览已变化");
    profile.initial ??= structuredClone(profile.current);
    if (data.airplaneFault) throw new Error(data.airplaneFault);
    profile.current.enabled = args.request.enabled;
  } else {
    if (args.expectedRevision !== profile.restoreRevision || !profile.initial || profile.restoreError) throw new Error(profile.restoreError || "初始恢复预览已变化");
    profile.current = structuredClone(profile.initial); profile.initial = null;
  }
  profile.revision += "-changed"; profile.restoreRevision = profile.initial ? profile.revision + "-restore" : null;
  return null;
}
