export function builtinCatalog(data) {
  const profiles = [
    { id: "quietOffice", name: "安静办公", description: "会议、文档和浏览" },
    { id: "mobileBattery", name: "移动续航", description: "外出时降低耗电" },
    { id: "pluggedGaming", name: "插电游戏", description: "接入符合机型要求的适配器后运行高负载游戏" },
  ];
  const panel = data.display.displays.find(display => display.current.identity === "isolated-panel");
  const candidates = panel?.frequencies ?? [];
  const managed = data.autoRefresh.initial != null;
  const refreshIssue = managed ? "内屏刷新率由「自动刷新率」管理" : candidates.length < 2 ? "未识别到支持多档刷新的活动内屏" : null;
  const refresh = refreshIssue ? null : {
    revision: JSON.stringify([data.builtinPanelRevision, { ...panel.current, frequency: 0 }, candidates]),
    current: panel.current.frequency, candidates, low: candidates[0], high: candidates.at(-1),
  };
  return { profiles, refresh, refreshIssue };
}

export function requireBuiltinPower(data, request) {
  if (request.id !== "pluggedGaming" || data.battery.power.acOnline === true) return;
  throw new Error(data.battery.power.acOnline === false
    ? "当前使用电池供电，请接入满足机型要求的电源适配器后重新预览"
    : "供电状态未知，请确认适配器已接入后重新预览");
}

export function builtinFile(data, request) {
  const catalog = builtinCatalog(data);
  const mode = catalog.profiles.findIndex(profile => profile.id === request.id);
  if (mode < 0) throw new Error("内置档案不存在");
  const settings = { ...emptyProfileSettings(), oemPowerMode: mode };
  if (request.refresh) {
    const { refresh, refreshIssue } = catalog;
    if (!refresh) throw new Error(refreshIssue);
    if (refresh.revision !== request.refresh.revision || !refresh.candidates.includes(request.refresh.officeHz)) throw new Error("内屏候选已变化");
    const panel = data.display.displays.find(display => display.current.identity === "isolated-panel");
    settings.display = { displays: [{ ...panel.current, frequency: [request.refresh.officeHz, refresh.low, refresh.high][mode] }] };
  }
  return { version: 1, name: catalog.profiles[mode].name, savedAtMs: 0, settings };
}

export async function previewBuiltinFixture(data, request) {
  const file = builtinFile(data, request);
  const review = await previewManualProfile(data, file.settings);
  try { requireBuiltinPower(data, request); }
  catch (error) { review.blocked = error.message; }
  return { file, review };
}
