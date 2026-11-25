export function invokeSupplyMode(data, command, args) {
  const view = data.supplyMode;
  if (command === "read_oem_supply_mode") {
    if (data.supplyModeReadError) throw new Error(data.supplyModeReadError);
    return { blockedReason: null, archiveRevision: null, ...structuredClone(view ?? { revision: "unsupported", review: null, enabled: false, canEnable: false, canRestore: false,
      recovery: null, error: null, recoveryError: null, listenerOnline: true, sleeping: false, active: false, canResume: false, issue: null, last: null }) };
  }
  if (!view) throw new Error("隔离供电模式不可用");
  if (command === "pause_oem_supply_mode") {
    view.active = false; view.canResume = view.enabled; return null;
  }
  if (command === "archive_invalid_oem_supply_record") {
    if (!view.archiveRevision || args.expectedRevision !== view.archiveRevision) throw new Error("无效供电记录已变化，请重新读取后确认归档");
    Object.assign(view, { revision: `supply-${++data.supplyModeRevision}`, enabled: false, active: false, archiveRevision: null, error: null, canEnable: !!view.review });
    return "oem-supply-archive-isolated/oem-supply-mode.json";
  }
  if (args.expectedRevision !== view.revision) throw new Error("供电模式预览已变化，请重新确认");
  if (data.supplyModeFault) throw new Error(data.supplyModeFault);
  if (command === "enable_oem_supply_mode" || command === "resume_oem_supply_mode") {
    const allowed = command === "enable_oem_supply_mode" ? view.canEnable : view.canResume;
    if (!allowed) throw new Error("隔离规则当前不可启用");
    view.enabled = true; view.active = true; view.canEnable = false; view.canResume = false; view.canRestore = true;
  } else if (command === "restore_oem_supply_mode") {
    if (!view.canRestore) throw new Error("隔离恢复条件不可用");
    view.enabled = false; view.active = false; view.canResume = false; view.canEnable = true; view.canRestore = false; view.recovery = null;
  } else throw new Error(`未声明的供电模式命令：${command}`);
  view.revision = `supply-${++data.supplyModeRevision}`;
  return null;
}
