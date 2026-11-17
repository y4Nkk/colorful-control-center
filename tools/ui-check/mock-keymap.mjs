export async function invokeKeymap(data, command, args) {
  if (command === "preview_keymap_import") {
    if (data.keymapImportFault) throw new Error(data.keymapImportFault);
    const file = JSON.parse(args.json);
    if (file.version !== 1 || !file.name?.trim() || file.configuration?.devices?.length !== 2) throw new Error("映射配置格式无效");
    return structuredClone(file);
  }
  if (command === "read_keymap") {
    if (data.keymapReadFault) throw new Error(data.keymapReadFault);
    return structuredClone(data.keymap);
  }
  if (command === "start_keymap_recording") {
    if (data.keymapRecordingDelay) await new Promise(resolve => setTimeout(resolve, data.keymapRecordingDelay));
    data.keymapCapture = { active: true, steps: [], reason: null, elapsedMs: 0 };
    return structuredClone(data.keymapCapture);
  }
  if (command === "read_keymap_recording") return structuredClone(data.keymapCapture);
  if (command === "stop_keymap_recording") {
    if (data.keymapCapture) data.keymapCapture = { ...data.keymapCapture, active: false, reason: "已停止录制" };
    return structuredClone(data.keymapCapture ?? null);
  }
  const profile = data.keymap, revision = args.request?.expectedRevision ?? args.expectedRevision;
  if (!profile || revision !== profile.revision) throw new Error("宏配置预览已变化");
  if (profile.blockedReason) throw new Error(profile.blockedReason);
  if (data.keymapCapture?.active) throw new Error("请先结束宏录制");
  if (command === "retry_keymap_programs") { profile.programs = { running:true, error:null }; return null; }
  profile.revision = `keymap-${++data.keymapRevision}`;
  data.initialKeymap ??= structuredClone(profile.configuration); profile.canRestore = true; profile.baselineName = "隔离原厂配置";
  if (data.keymapFault) { const error = data.keymapFault; data.keymapFault = null; profile.recoveryRequired = true; throw new Error(error); }
  if (command === "restore_keymap") {
    profile.configuration = structuredClone(data.initialKeymap); profile.recoveryRequired = false;
  } else {
    for (const device of args.request.configuration.devices) {
      const index = profile.configuration.devices.findIndex(group => group.device === device.device);
      profile.configuration.devices[index] = structuredClone(device);
    }
  }
  profile.enabledFlags = ["keyboard", "mouse"].map(device => Number(profile.configuration.devices.find(group => group.device === device).enabled));
  profile.programsRequired = profile.configuration.devices.some(group => group.enabled && group.bindings.some(binding => binding.action.kind === "launch"));
  profile.programs = { running:profile.programsRequired, error:null };
  return null;
}
