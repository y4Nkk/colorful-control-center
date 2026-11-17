export function invokeLightBar(data, command, args) {
  if (command === "read_light_bar") {
    if (data.lightBarReadFault) throw new Error(data.lightBarReadFault);
    return structuredClone(data.lightBar ?? null);
  }
  const profile = data.lightBar;
  if (!profile || profile.blockedReason) throw new Error(profile?.blockedReason ?? "没有独立灯条");
  const revision = command === "apply_light_bar" ? args.request.expectedRevision : args.expectedRevision;
  if (revision !== profile.revision) throw new Error("灯条预览已变化，请重新读取确认");
  if (command === "apply_light_bar" && (!profile.current || args.request.values.kind !== profile.kind)) throw new Error("灯条型号或当前值不允许该目标");
  if (command === "restore_light_bar" && profile.initial?.kind !== profile.kind) throw new Error("灯条初始型号与设备不一致");
  profile.initial ??= structuredClone(profile.current);
  data.lightBarInitialPresets ??= structuredClone(profile.presets);
  if (data.lightBarFault) { const error = data.lightBarFault; data.lightBarFault = null; throw new Error(error); }
  profile.current = structuredClone(command === "apply_light_bar" ? args.request.values : profile.initial);
  if (command === "restore_light_bar") profile.presets = structuredClone(data.lightBarInitialPresets);
  else {
    const target = profile.current;
    let found = false;
    for (const preset of profile.presets) {
      if (preset.kind !== target.kind) continue;
      preset.settings.brightness = target.settings.brightness; preset.settings.speed = target.settings.speed;
      if (target.kind === "x170") preset.settings.enabled = target.settings.enabled;
      if (preset.settings.effect === target.settings.effect) { preset.settings = structuredClone(target.settings); found = true; }
    }
    if (!found) profile.presets.push(structuredClone(target));
  }
  data.lightBarRevision = (data.lightBarRevision ?? 0) + 1; profile.revision = `lightbar-${data.lightBarRevision}`;
  return null;
}
