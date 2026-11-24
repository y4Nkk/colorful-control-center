export function invokeAirplane(data, command, args) {
  data.airplane ??= { revision: "airplane-0", identity: "isolated-airplane", current: { device: "airplane-A", enabled: false }, error: null, blockedReason: null };
  const profile = data.airplane;
  if (command === "read_airplane") return structuredClone(profile);
  if (command !== "apply_airplane") throw new Error("未知飞行模式命令：" + command);
  if (profile.revision !== args.request.expectedRevision || !profile.current) throw new Error("飞行模式预览已变化");
  if (data.airplaneFault) throw new Error(data.airplaneFault);
  profile.current.enabled = args.request.enabled;
  profile.revision += "-changed";
  return null;
}
