export function invokeLightingOptions(data, command, args) {
  const options = data.lightingOptions;
  if (command === "read_lighting_options") {
    if (data.lightingOptionsReadFault) throw new Error(data.lightingOptionsReadFault);
    return structuredClone(options);
  }
  if (command === "restore_lighting_options") {
    if (args.expectedRevision !== options.revision || !options.initial) throw new Error("灯光恢复预览已变化");
    if (data.lightingOptionsRestoreFault) throw new Error(data.lightingOptionsRestoreFault);
    options.current = structuredClone(options.initial);
  } else if (command === "apply_lighting_options") {
    const request = args.request;
    if (request.expectedRevision !== options.revision) throw new Error("灯光选项预览已变化");
    for (const key of ["bootOverride", "logo", "lightBar", "topCover"]) {
      if ((request.values[key] !== null) !== options.capabilities[key]) throw new Error("灯光能力已变化");
    }
    if (request.values.topCover && (!Number.isInteger(request.values.topCover.brightness) || request.values.topCover.brightness < 0 || request.values.topCover.brightness > 4)) throw new Error("顶盖亮度越界");
    if (JSON.stringify(request.values) === JSON.stringify(options.current)) return null;
    options.initial ??= structuredClone(options.current); options.canRestore = true;
    options.revision = `lighting-options-${++data.lightingOptionsRevision}`;
    if (data.lightingOptionsFault) { const message = data.lightingOptionsFault; data.lightingOptionsFault = null; throw new Error(message); }
    options.current = structuredClone(request.values);
    return null;
  } else throw new Error(`未知灯光选项命令：${command}`);
  options.revision = `lighting-options-${++data.lightingOptionsRevision}`;
  return null;
}
export function applyProfileLightingOptions(data, settings) {
  const target = settings.lightingOptions;
  if (!target) return false;
  if (target.keyboardType !== data.lightingOptions.capabilities.keyboardType) throw new Error("档案灯光设备已变化");
  if (JSON.stringify(target.values) === JSON.stringify(data.lightingOptions.current)) return false;
  invokeLightingOptions(data, "apply_lighting_options", { request: { expectedRevision: data.lightingOptions.revision, values: target.values } });
  return true;
}
