export function previewProfileDisplay(data, settings) {
  if (!settings.display) return null;
  return { displays: settings.display.displays.map(saved => {
    const actual = data.display.displays.find(display => display.current.device === saved.device);
    if (!actual) throw new Error("档案显示器已断开");
    const normalized = { ...saved, frequency: actual.current.frequency, name: actual.current.name };
    if (JSON.stringify(normalized) !== JSON.stringify(actual.current)) throw new Error("档案显示器身份或几何已变化");
    if (!actual.frequencies.includes(saved.frequency)) throw new Error("档案刷新率已不受支持");
    return { before: structuredClone(actual.current), target: { ...actual.current, frequency: saved.frequency } };
  }) };
}
export function profileApplicationSteps(data) {
  const s = data.profile.settings;
  return data.profileSteps ?? [[s.lanPolicy, "网络驱动策略"], [s.performanceOptions, "性能选项"],
    [s.batteryCharge ?? s.batteryEnergy, "电池配置"], [s.powerDelivery, "PD档位"], [s.usbPolicy, "USB选择性暂停"],
    [s.multiZoneLighting, "多区键盘灯效"], [s.lightBar, "独立灯条"], [s.dustSchedule, "除尘计划"], [s.microphones, "麦克风静音"], [s.touchpad, "触摸板开关"], [s.keyboard, "键盘功能"], [s.keyboardLighting, "键盘灯效"],
    [s.lightingOptions, "灯光策略与外围灯区"], [s.perkeyLighting, "逐键灯效"], [s.keymap, "按键映射与宏"],
    [s.keymap, "提交映射与程序监听"], [s.display, "显示刷新率复核"], [s.gpuPower, "独显目标复核"], [s.graphicsMode, "显卡模式选择请求（重启后核对）"], [s.gpu, "GPU 偏移"]]
    .filter(([value]) => value != null).map(([, step]) => ({ step, ok: true, message: "已确认" }));
}
export function recoverProfileDisplay(data) {
  const errors = [];
  for (const change of data.profileApplicationSession.review.displays) {
    const actual = data.display.displays.find(display => display.current.device === change.before.device);
    if (!actual) { errors.push("显示器已断开"); continue; }
    if (JSON.stringify(actual.current) === JSON.stringify(change.before)) continue;
    if (JSON.stringify(actual.current) !== JSON.stringify(change.target)) { errors.push("显示模式已被外部修改，未覆盖"); continue; }
    actual.current = structuredClone(change.before);
  }
  if (errors.length) data.profileApplication.pending.remainingMs = null;
  else { data.profileApplication.pending = null; data.display.pending = null; }
  return { step: "恢复本次显示变更", ok: !errors.length, message: errors.join("；") || "已恢复" };
}
export function syncProfileApplication(data) {
  const report = data.profileApplication;
  if (report?.pending?.remainingMs != null) {
    report.pending.remainingMs = Math.max(0, data.profileTrialDeadline - Date.now());
    if (!report.pending.remainingMs) {
      report.revision += 1;
      report.steps.push({ step: "显示试用结束", ok: false, message: "试用已恢复，未执行其余档案设置" }, recoverProfileDisplay(data));
    }
  }
  return structuredClone(report ?? null);
}
export async function beginProfileApplication(data, args) {
  if (data.profileApplication && ["ac", "battery", "resume"].includes(data.profileApplication.origin)) data.lastAutomaticProfileApplication = structuredClone(data.profileApplication);
  if (syncProfileApplication(data)?.pending) throw new Error("先结束当前档案显示确认或恢复");
  if (JSON.stringify(args.expected) !== JSON.stringify(data.profile)) throw new Error("档案已变化，请重新预览后确认");
  if (JSON.stringify(args.expectedMode) !== JSON.stringify(await previewProfileMode(data, args.expected.settings))) throw new Error("模式联动已变化，请重新预览");
  const review = previewProfileDisplay(data, args.expected.settings);
  if (JSON.stringify(args.expectedDisplay) !== JSON.stringify(review)) throw new Error("档案显示预览已变化");
  const gpuReview = previewProfileGpuPower(data, args.expected.settings);
  if (JSON.stringify(args.expectedGpuPower) !== JSON.stringify(gpuReview)) throw new Error("档案独显预览已变化");
  const muxReview = previewProfileGraphicsMode(data, args.expected.settings);
  if (JSON.stringify(args.expectedGraphicsMode) !== JSON.stringify(muxReview)) throw new Error("档案显卡模式预览已变化");
  reviewProfileLan(data, args.expected.settings.lanPolicy);
  const id = (data.profileApplication?.id ?? 0) + 1;
  data.profileApplication = { id, revision: 1, origin: "manual", targetsApplied: false, name: args.expected.name, steps: [], pending: null };
  const changes = review?.displays.filter(change => change.before.frequency !== change.target.frequency) ?? [];
  if (changes.length) {
    data.profileApplicationSession = { file: structuredClone(args.expected), mode: structuredClone(args.expectedMode), review, gpuReview, muxReview };
    for (const change of changes) data.display.displays.find(display => display.current.device === change.target.device).current = structuredClone(change.target);
    data.profileTrialDeadline = Date.now() + 15_000;
    data.profileApplication.pending = { before: changes.map(change => change.before), targets: changes.map(change => change.target), rotation: false, automatic: false, trialId: id, remainingMs: 15_000, profileSession: id, error: null };
    data.display.pending = data.profileApplication.pending;
  } else {
    await commitProfileTargets(data);
  }
  return structuredClone(data.profileApplication);
}
export async function invokeProfileApplication(data, command, args) {
  if (command === "preview_profile_display") return previewProfileDisplay(data, args.settings);
  if (command === "preview_profile_graphics_mode") return previewProfileGraphicsMode(data, args.settings);
  if (command === "preview_profile_gpu_power") return previewProfileGpuPower(data, args.settings);
  if (command === "read_profile_application") return syncProfileApplication(data);
  if (command === "apply_profile") return beginProfileApplication(data, args);
  const report = syncProfileApplication(data);
  if (!report?.pending || report.id !== args.id) throw new Error("档案显示会话已结束或不属于此请求");
  data.profileApplication.revision += 1;
  if (command === "cancel_profile_application") {
    data.profileApplication.targetsApplied = false;
    data.profileApplication.steps.push({ step: "取消档案应用", ok: false, message: "其余步骤未执行" }, recoverProfileDisplay(data));
  } else {
    try {
      const session = data.profileApplicationSession;
      if (report.steps.some(step => !step.ok)) throw new Error("请先恢复本次显示变更");
      if (JSON.stringify(data.profile) !== JSON.stringify(session.file)) throw new Error("档案已变化");
      if (JSON.stringify(await previewProfileMode(data, session.file.settings)) !== JSON.stringify(session.mode)) throw new Error("模式联动已变化");
      reviewProfileLan(data, session.file.settings.lanPolicy);
      if (JSON.stringify(previewProfileGpuPower(data, session.file.settings)) !== JSON.stringify(session.gpuReview)) throw new Error("独显预览已变化");
      if (JSON.stringify(previewProfileGraphicsMode(data, session.file.settings)) !== JSON.stringify(session.muxReview)) throw new Error("显卡模式预览已变化");
      previewProfileDisplay(data, session.file.settings);
      await commitProfileTargets(data);
      if (!data.profileApplication.targetsApplied) data.profileApplication.steps.push(recoverProfileDisplay(data));
      else { data.profileApplication.pending = null; data.display.pending = null; }
    } catch (error) {
      data.profileApplication.steps.push({ step: "保留显示与重新预检", ok: false, message: String(error) }, recoverProfileDisplay(data));
    }
  }
  return structuredClone(data.profileApplication);
}
